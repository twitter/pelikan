#include <core/worker.h>

#include <core/shared.h>

#include <protocol/memcache_include.h>
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

#define WORKER_MODULE_NAME "core::worker"

static bool worker_init = false;
worker_metrics_st *worker_metrics = NULL;

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

static inline rstatus_i
_worker_write(struct buf_sock *s)
{
    rstatus_i status;

    log_verb("writing on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    status = buf_tcp_write(s);

    return status;
}

static inline void
_worker_post_write(struct buf_sock *s)
{
    log_verb("post write processing on buf_sock %p", s);

    /* left-shift rbuf and wbuf */
    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);

    dbuf_shrink(&(s->rbuf));
    dbuf_shrink(&(s->wbuf));
}

static inline void
_worker_event_write(struct buf_sock *s)
{
    rstatus_i status;
    struct tcp_conn *c = s->ch;

    status = _worker_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
        event_add_write(ctx->evb, hdl->wid(c), s);
    } else if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
    }
    _worker_post_write(s);
}

static inline void
_worker_read(struct buf_sock *s)
{
    log_verb("reading on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    /* TODO(kyang): consider refactoring dbuf_tcp_read and buf_tcp_read to have no return status
       at all, since the return status is already given by the connection state */
    dbuf_tcp_read(s);
}

static inline void
worker_close(struct buf_sock *s)
{
    log_info("worker core close on buf_sock %p", s);

    event_deregister(ctx->evb, s->ch->sd);
    hdl->term(s->ch);
    buf_sock_return(&s);
}

static inline void
_post_read(struct buf_sock *s)
{
    parse_rstatus_t status;
    struct request *req;
    struct response *rsp = NULL;

    log_verb("post read processing on buf_sock %p", s);

    if (s->data == NULL) {
        s->data = request_borrow();
    }

    req = s->data;

    if (req == NULL) {
        /* TODO(yao): close the connection for now, we should write a OOM
         * message and send it to the client later.
         */
        log_error("cannot acquire request: OOM");

        goto error;
    }

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(s->rbuf) > 0) {
        struct response *nr;
        int i, n, card;

        /* parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(s->rbuf));

        status = parse_req(req, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }

        if (status != PARSE_OK) {
            /* parsing errors are all client errors, since we don't know
             * how to recover from client errors in this condition (we do not
             * have a valid request so we don't know where the invalid request
             * ends), we should close the connection
             */
            log_info("illegal request received, status: %d", status);

            goto error;
        }

        /* processing */
        if (req->type == REQ_QUIT) {
            log_info("peer called quit");
            s->ch->state = CHANNEL_TERM;
            goto done;
        }

        /* find cardinality of the request and get enough response objects */
        card = array_nelem(req->keys);
        if (req->type == REQ_GET || req->type == REQ_GETS) {
            /* extra response object for the "END" line after values */
            card++;
        }
        rsp = response_borrow();
        for (i = 1, nr = rsp; i < card; i++) {
            STAILQ_NEXT(nr, next) = response_borrow();
            nr = STAILQ_NEXT(nr, next);
            if (nr == NULL) {
                log_error("cannot borrow enough rsp objects, close channel");

                goto error;
            }
        }
        /* actual handling */
        process_request(rsp, req);

        klog_write(req, rsp);

        /* writing results */
        if (req->noreply) { /* noreply means no writing to buffers */
            request_reset(req);
            continue;
        }

        nr = rsp;
        if (req->type == REQ_GET || req->type == REQ_GETS) {
            i = req->nfound;
            /* process returns an extra rsp which accounts for RSP_END */
            while (i > 0) {
                n = compose_rsp(&s->wbuf, nr);
                if (n < 0) {
                    log_error("composing rsp erred, terminate channel");

                    goto error;
                }
                nr = STAILQ_NEXT(nr, next);
                i--;
            }
        }
        n = compose_rsp(&s->wbuf, nr);
        if (n < 0) {
            log_error("composing rsp erred, terminate channel");

            goto error;
        }

        /* clean up resources */
        request_reset(req);
        response_return_all(&rsp);

        ASSERT(rsp == NULL);
    }

done:
    /* TODO: call stream write directly to save one event */
    if (buf_rsize(s->wbuf) > 0) {
        log_verb("adding write event");
        _worker_event_write(s);
    }

    return;

error:
    request_return(&req);
    response_return_all(&rsp);
    s->ch->state = CHANNEL_TERM;
}

/* read event over an existing connection */
static inline void
_worker_event_read(struct buf_sock *s)
{
    ASSERT(s != NULL);

    _worker_read(s);
    _post_read(s);
}

static void
worker_add_conn(void)
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
        log_verb("Adding new buf_sock %p to worker thread", s);
        s->owner = ctx;
        s->hdl = hdl;
        event_add_read(ctx->evb, hdl->rid(s->ch), s);
    }
}

static void
core_worker_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;
    log_verb("worker event %06"PRIX32" on buf_sock %p", events, s);

    if (s == NULL) {
        /* event on pipe_c, new connection */
        if (events & EVENT_READ) {
            worker_add_conn();
        } else if (events & EVENT_ERR) {
            log_error("error event received on conn_fds pipe");
        } else {
            /* there should never be any write events on the pipe from worker */
            NOT_REACHED();
        }
    } else {
        /* event on one of the connections */

        if (events & EVENT_READ) {
            log_verb("processing worker read event on buf_sock %p", s);
            INCR(worker_metrics, worker_event_read);
            _worker_event_read(s);
        } else if (events & EVENT_WRITE) {
            log_verb("processing worker write event on buf_sock %p", s);
            INCR(worker_metrics, worker_event_write);
            _worker_event_write(s);
        } else if (events & EVENT_ERR) {
            s->ch->state = CHANNEL_TERM;
            INCR(worker_metrics, worker_event_error);
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
            request_return((struct request **)&s->data);
            worker_close(s);
        }
    }
}

rstatus_i
core_worker_setup(worker_metrics_st *metrics)
{
    if (worker_init) {
        log_error("worker has already been setup, aborting");

        return CC_ERROR;
    }

    log_info("set up the %s module", WORKER_MODULE_NAME);

    ctx->timeout = 100;
    ctx->evb = event_base_create(1024, core_worker_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup worker thread core; could not create event_base");
        return CC_ERROR;
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
    worker_metrics = metrics;
    if (metrics != NULL) {
        WORKER_METRIC_INIT(worker_metrics);
    }

    worker_init = true;

    return CC_OK;
}

void
core_worker_teardown(void)
{
    log_info("tear down the %s module", WORKER_MODULE_NAME);

    if (!worker_init) {
        log_warn("%s has never been setup", WORKER_MODULE_NAME);
    } else {
        event_base_destroy(&(ctx->evb));
    }
    worker_metrics = NULL;
    worker_init = false;
}

static rstatus_i
core_worker_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(worker_metrics, worker_event_loop);
    INCR_N(worker_metrics, worker_event_total, n);
    time_update();

    return CC_OK;
}

void *
core_worker_evloop(void *arg)
{
    rstatus_i status;

    for(;;) {
        status = core_worker_evwait();
        if (status != CC_OK) {
            log_crit("worker core event loop exited due to failure");
            break;
        }
    }

    exit(1);
}
