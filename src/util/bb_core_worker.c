#include <util/bb_core_worker.h>

#include <bb_stats.h>
#include <time/bb_time.h>
#include <util/bb_core_shared.h>

#if defined TARGET_SLIMCACHE
#include <slimcache/bb_process.h>
#elif defined TARGET_TWEMCACHE
#include <twemcache/bb_process.h>
#endif

#include <cc_channel.h>
#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>

#include <stream/cc_sockio.h>

static struct context context;
static struct context *ctx = &context;

static channel_handler_t handlers;
static channel_handler_t *hdl = &handlers;

static void
_worker_close(struct buf_sock *s)
{
    log_info("worker core close on buf_sock %p", s);

    event_deregister(ctx->evb, s->ch->sd);

    hdl->term(s->ch);
    request_return((struct request **)&s->data);
    buf_sock_return(&s);
}

static rstatus_t
_worker_write(struct buf_sock *s)
{
    rstatus_t status;

    log_verb("writing on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    status = buf_tcp_write(s);

    return status;
}

static void
_worker_post_write(struct buf_sock *s)
{
    log_verb("post write processing on buf_sock %p", s);

    //stats_thread_incr_by(data_written, nbyte);

    /* left-shift rbuf and wbuf */
    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);
}

static void
_worker_event_write(struct buf_sock *s)
{
    rstatus_t status;
    struct conn *c = s->ch;

    status = _worker_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
        event_add_write(ctx->evb, hdl->id(c), s);
    } else if (status == CC_ERROR) {
        c->state = CONN_CLOSING;
    }
    _worker_post_write(s);
}

static rstatus_t
_worker_read(struct buf_sock *s)
{
    log_verb("reading on buf_sock %p", s);

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    rstatus_t status;

    status = buf_tcp_read(s);
    if (status == CC_ENOMEM) {
        log_debug("not enough room in rbuf: start %p, rpos %p, wpos %p, end %p",
                s->rbuf->begin, s->rbuf->rpos, s->rbuf->wpos, s->rbuf->end);
        status = CC_ERETRY; /* retry when we cannot read due to buffer full */
    }

    return status;
}

static void
_worker_post_read(struct buf_sock *s)
{
    rstatus_t status;
    struct request *req;

    log_verb("post read processing on buf_sock %p", s);

    //stats_thread_incr_by(data_read, nbyte);

    if (s->data != NULL) {
        req = s->data;
    } else {
        req = request_borrow();
        s->data = req;
    }

    if (req == NULL) {
        log_error("cannot acquire request: OOM");
        status = compose_rsp_msg(s->wbuf, RSP_SERVER_ERROR, false);
        if (status != CC_OK) {
            //s->err = status;

            log_error("failed to send server error, status: %d", status);
        }

        goto done;
    }

    if (req->swallow) {
        status = parse_swallow(s->rbuf);
        if (status == CC_OK) {
            request_reset(req);
        } else { /* CC_UNFIN */
            goto done;
        }
    }

    while (buf_rsize(s->rbuf) > 0) {
        /* parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(s->rbuf));

        status = parse_req(req, s->rbuf);
        if (status == CC_UNFIN) {
            goto done;
        }

        if (status != CC_OK) { /* parsing errors are all client errors */
            log_warn("illegal request received, status: %d", status);

            status = compose_rsp_msg(s->wbuf, RSP_CLIENT_ERROR, false);
            if (status != CC_OK) {
                log_error("failed to send client error, status: %d", status);
            }

            goto done;
        }

        /* processing */
        log_verb("wbuf free: %"PRIu32" B", buf_wsize(s->wbuf));
        status = process_request(req, s->wbuf);
        log_verb("wbuf free: %"PRIu32" B", buf_wsize(s->wbuf));

        if (status == CC_ENOMEM) {
            log_debug("wbuf full, try again later");
            goto done;
        }
        if (status == CC_ERDHUP) {
            log_info("peer called quit");
            s->ch->state = CONN_CLOSING;
            goto done;
        }

        if (status != CC_OK) {
            log_error("process request failed for other reason: %d", status);

            status = compose_rsp_msg(s->wbuf, RSP_SERVER_ERROR, false);
            if (status != CC_OK) {
                /* NOTE(yao): this processing logic does NOT work for large
                 * values, which will easily overflow wbuf and therefore always
                 * fail. Here we can do this because the values are very small
                 * relative to the size wbuf.
                 *
                 * The right way of handling write of any size value is to copy
                 * data directly from our data store on heap to the channel.
                 *
                 * If we want to be less aggressive in raising errors, we can
                 * re-process the current request when wbuf is full. This will
                 * require small modification to this function and struct request.
                 */
                log_error("failed to send server error, status: %d", status);
            }

            goto done;
        }

        request_reset(req);
    }

done:
    /* TODO: call stream write directly to save one event */
    if (buf_rsize(s->wbuf) > 0) {
        _worker_event_write(s);
    }
}

/* read event over an existing connection */
static void
_worker_event_read(struct buf_sock *s)
{
    rstatus_t status;
    struct conn *c;

    ASSERT(s != NULL);

    c = s->ch;
    status = _worker_read(s);
    if (status == CC_ERROR) {
        c->state = CONN_CLOSING;
    }

    _worker_post_read(s);
}

/* read event over conn_fds pipe, signaling new connection */
static void
_worker_add_conn(void)
{
    struct buf_sock *s;
    char buf[RING_ARRAY_DEFAULT_CAP]; /* buffer for discarding pipe data */

    while (ring_array_pop(&s, conn_arr) == CC_OK) {
        log_verb("Adding new buf_sock %p to worker thread", s);
        s->owner = ctx;
        s->hdl = hdl;
        event_add_read(ctx->evb, hdl->id(s->ch), s);
    }

    read(conn_fds[0], buf, RING_ARRAY_DEFAULT_CAP);
}

static void
core_worker_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;

    log_verb("worker event %06"PRIX32" on buf_sock %p", events, s);

    if (s == NULL) {
        /* event on conn_fds pipe, new connection */

        if (events & EVENT_READ) {
            _worker_add_conn();
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
            INCR(worker_event_read);
            _worker_event_read(s);
        } else if (events & EVENT_WRITE) {
            log_verb("processing worker write event on buf_sock %p", s);
            INCR(worker_event_write);
            _worker_event_write(s);
        } else if (events & EVENT_ERR) {
            INCR(worker_event_error);
            _worker_close(s);
        } else {
            NOT_REACHED();
        }

        if (s->ch->state == CONN_CLOSING ||
            (s->ch->state == CONN_EOF && buf_rsize(s->wbuf) == 0)) {
            _worker_close(s);
        }
    }
}

rstatus_t
core_worker_setup(void)
{
    ctx->timeout = 100;
    ctx->evb = event_base_create(1024, core_worker_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup worker thread core; could not create event_base");
        return CC_ERROR;
    }

    hdl->accept = tcp_accept;
    hdl->reject = tcp_reject;
    hdl->open = tcp_listen;
    hdl->term = tcp_close;
    hdl->recv = tcp_recv;
    hdl->send = tcp_send;
    hdl->id = conn_id;

    event_add_read(ctx->evb, conn_fds[0], NULL);

    return CC_OK;
}

void
core_worker_teardown(void)
{
    event_base_destroy(&(ctx->evb));
}

static rstatus_t
core_worker_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(worker_event_loop);
    INCR_N(worker_event_total, n);
    time_update();

    return CC_OK;
}

void *
core_worker_evloop(void *arg)
{
    rstatus_t status;

    for(;;) {
        status = core_worker_evwait();
        if (status != CC_OK) {
            log_crit("worker core event loop exited due to failure");
            break;
        }
    }

    exit(1);
}
