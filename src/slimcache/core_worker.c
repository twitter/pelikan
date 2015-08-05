#include <slimcache/process.h>

#include <core/worker.h>
#include <protocol/memcache/codec.h>
#include <protocol/memcache/klog.h>
#include <protocol/memcache/request.h>

#include <buffer/cc_buf.h>
#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_channel.h>
#include <channel/cc_pipe.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

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

    /* left-shift rbuf and wbuf */
    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);
}

static void
_worker_event_write(struct buf_sock *s)
{
    rstatus_t status;
    struct tcp_conn *c = s->ch;

    status = _worker_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
        worker_retry_write(s, c);
    } else if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
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
    uint32_t rsp_len = 0;

    log_verb("post read processing on buf_sock %p", s);

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
        rsp_len = process_request(req, s->wbuf);

        if (rsp_len < 0) {
            status = rsp_len;
            rsp_len = 0;
        } else {
            status = 0;
        }

        klog_write(req, status, rsp_len);

        if (status == CC_ENOMEM) {
            log_debug("wbuf full, try again later");
            goto done;
        }
        if (status == CC_ERDHUP) {
            log_info("peer called quit");
            s->ch->state = CHANNEL_TERM;
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
    struct tcp_conn *c;

    ASSERT(s != NULL);

    c = s->ch;
    status = _worker_read(s);
    if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
    }

    _worker_post_read(s);
}

void
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
            INCR(worker_metrics, worker_event_error);
            worker_close(s);
        } else {
            NOT_REACHED();
        }

        if (s->ch->state == CHANNEL_TERM && buf_rsize(s->wbuf) == 0) {
            worker_close(s);
        }
    }
}
