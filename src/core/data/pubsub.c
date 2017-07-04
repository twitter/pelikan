#include "pubsub.h"

#include "shared.h"

#include <time/time.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_channel.h>
#include <channel/cc_pipe.h>
#include <channel/cc_tcp.h>

#include <stream/cc_sockio.h>

#include <sysexits.h>

#define PUBSUB_MODULE_NAME "core::pubsub"

pubsub_metrics_st *pubsub_metrics = NULL;

static struct context context;
struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

struct pubsub_processor *processor;

static inline rstatus_i
_pubsub_write(struct buf_sock *s)
{
    rstatus_i status;

    log_verb("writing on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    status = buf_tcp_write(s);

    return status;
}

/* the caller only needs to check the return status of this function if
 * it previously received a write event and wants to re-register the
 * read event upon full, successful write.
 */
static inline rstatus_i
_pubsub_event_write(struct buf_sock *s)
{
    rstatus_i status;
    struct tcp_conn *c = s->ch;

    status = _pubsub_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
        /* by removing current masks and only listen to write event(s), we are
         * effectively stopping processing incoming data until we can write
         * something to the (kernel) buffer for the channel. This is sensible
         * because either the local network or the client is backed up when
         * kernel write buffer is full, and this allows us to propagate back
         * pressure to the sending side.
         */

        event_del(ctx->evb, hdl->wid(c));
        event_add_write(ctx->evb, hdl->wid(c), s);
    } else if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
    }

    if (processor->write(s) < 0) {
        log_debug("handler signals channel termination");
        s->ch->state = CHANNEL_TERM;
        return CC_ERROR;
    }

    return status;
}

static inline void
_pubsub_read(struct buf_sock *s)
{
    log_verb("reading on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    /* TODO(kyang): consider refactoring dbuf_tcp_read and buf_tcp_read to have no return status
       at all, since the return status is already given by the connection state */
    buf_tcp_read(s);
}

static inline void
pubsub_close(struct buf_sock *s)
{
    log_info("pubsub core close on buf_sock %p", s);

    processor->error(s);
    event_del(ctx->evb, hdl->rid(s->ch));
    hdl->term(s->ch);
    buf_sock_return(&s);
}

/* read event over an existing connection */
static inline void
_pubsub_event_read(struct buf_sock *s)
{
    ASSERT(s != NULL);

    _pubsub_read(s);
    if (processor->read(s) < 0) {
        log_debug("handler signals channel termination");
        s->ch->state = CHANNEL_TERM;
        return;
    }
    if (buf_rsize(s->wbuf) > 0) {
        log_verb("attempt to write");
        _pubsub_event_write(s);
    }
}

static void
pubsub_add_conn(void)
{
    struct buf_sock *s;
    char buf[RING_ARRAY_DEFAULT_CAP]; /* buffer for discarding pipe data */
    int i;
    rstatus_i status;

    /* server pushes connection on to the ring array before writing to the pipe,
     * therefore, we should read from the pipe first and take the connections
     * off the ring array to match the number of bytes received.
     *
     * Once we move server to its own thread, it is possible that there are more
     * connections added to the queue when we are processing, it is OK to wait
     * for the next read event in that case.
     */

    i = pipe_recv(pipe_c, buf, RING_ARRAY_DEFAULT_CAP);
    if (i < 0) { /* errors, do not read from ring array */
        log_warn("not adding new connections due to pipe error");
        return;
    }

    /* each byte in the pipe corresponds to a new connection, which we will
     * now get from the ring array
     */
    for (; i > 0; --i) {
        status = ring_array_pop(&s, conn_arr);
        if (status != CC_OK) {
            log_warn("event number does not match conn queue: missing %d conns",
                    i);
            return;
        }
        log_verb("Adding new buf_sock %p to pubsub thread", s);
        s->owner = ctx;
        s->hdl = hdl;
        event_add_read(ctx->evb, hdl->rid(s->ch), s);
    }
}

static void
_pubsub_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;
    log_verb("pubsub event %06"PRIX32" on buf_sock %p", events, s);

    if (s == NULL) {
        /* event on pipe_c, new connection */
        if (events & EVENT_READ) {
            pubsub_add_conn();
        } else if (events & EVENT_ERR) {
            log_error("error event received on conn_fds pipe");
        } else {
            /* there should never be any write events on the pipe from pubsub */
            NOT_REACHED();
        }
    } else {
        /* event on one of the connections */

        if (events & EVENT_READ) {
            log_verb("processing pubsub read event on buf_sock %p", s);
            INCR(pubsub_metrics, pubsub_event_read);
            _pubsub_event_read(s);
        } else if (events & EVENT_WRITE) {
            log_verb("processing pubsub write event on buf_sock %p", s);
            INCR(pubsub_metrics, pubsub_event_write);
            if (_pubsub_event_write(s) == CC_OK) {
                /* write backlog cleared up, re-add read event (only) */
                event_del(ctx->evb, hdl->wid(s->ch));
                event_add_read(ctx->evb, hdl->rid(s->ch), s);
            }
        } else if (events & EVENT_ERR) {
            s->ch->state = CHANNEL_TERM;
            INCR(pubsub_metrics, pubsub_event_error);
        } else {
            NOT_REACHED();
        }

        /* TODO(yao): come up with a robust policy about channel connection
         * and pending data. Since an error can either be server (usually
         * memory) issues or client issues (bad syntax etc), or requested (quit)
         * it is hard to determine whether the channel should be immediately
         * closed or not. A simplistic approach might be to always close asap,
         * and clients should not initiate closing unless they have received all
         * their responses. This is not as nice as the TCP half-close behavior,
         * but simpler to implement and probably fine initially.
         */
        if (s->ch->state == CHANNEL_TERM || s->ch->state == CHANNEL_ERROR) {
            pubsub_close(s);
        }
    }
}

void
core_pubsub_setup(pubsub_options_st *options, pubsub_metrics_st *metrics)
{
    int timeout = PUBSUB_TIMEOUT;
    int nevent = PUBSUB_NEVENT;

    log_info("set up the %s module", PUBSUB_MODULE_NAME);

    if (pubsub_init) {
        log_warn("pubsub has already been setup, re-creating");
        core_pubsub_teardown();
    }

    pubsub_metrics = metrics;

    if (options != NULL) {
        timeout = option_uint(&options->pubsub_timeout);
        nevent = option_uint(&options->pubsub_nevent);
    }

    ctx->timeout = timeout;
    ctx->evb = event_base_create(nevent, _pubsub_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup pubsub thread core; could not create event_base");
        exit(EX_CONFIG);
    }

    hdl->accept = (channel_accept_fn)tcp_accept;
    hdl->reject = (channel_reject_fn)tcp_reject;
    hdl->open = (channel_open_fn)tcp_listen;
    hdl->term = (channel_term_fn)tcp_close;
    hdl->recv = (channel_recv_fn)tcp_recv;
    hdl->send = (channel_send_fn)tcp_send;
    hdl->rid = (channel_id_fn)tcp_read_id;
    hdl->wid = (channel_id_fn)tcp_write_id;

    event_add_read(ctx->evb, pipe_read_id(pipe_c), NULL);

    pubsub_init = true;
}

void
core_pubsub_teardown(void)
{
    log_info("tear down the %s module", PUBSUB_MODULE_NAME);

    if (!pubsub_init) {
        log_warn("%s has never been setup", PUBSUB_MODULE_NAME);
    } else {
        event_base_destroy(&(ctx->evb));
    }
    pubsub_metrics = NULL;
    pubsub_init = false;
}

static rstatus_i
_pubsub_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(pubsub_metrics, pubsub_event_loop);
    INCR_N(pubsub_metrics, pubsub_event_total, n);
    time_update();

    return CC_OK;
}

void *
core_pubsub_evloop(void *arg)
{
    processor = arg;

    for(;;) {
        if (_pubsub_evwait() != CC_OK) {
            log_crit("pubsub core event loop exited due to failure");
            break;
        }
    }

    exit(1);
}
