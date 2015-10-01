#include <slimcache/process.h>

#include <core/worker.h>
#include <protocol/memcache_include.h>

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
    parse_rstatus_t status;
    struct request *req;

    log_verb("post read processing on buf_sock %p", s);

    if (s->data != NULL) {
        req = s->data;
    } else {
        req = request_borrow();
        s->data = req;
    }

    if (req == NULL) {
        /* TODO(yao): close the connection for now, we should write a OOM
         * message and send it to the client later.
         */
        log_error("cannot acquire request: OOM");

        goto error;
    }

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(s->rbuf) > 0) {
        struct response *rsp, *nr;
        int i, n, card;

        /* parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(s->rbuf));

        status = parse_req(req, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }

        if (status != CC_OK) {
            /* parsing errors are all client errors, since we don't know
             * how to recover from client errors in this condition (we do not
             * have a valid request so we don't know where the invalid request
             * ends), we should close the connection
             */
            log_warn("illegal request received, status: %d", status);

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
                log_debug("cannot borrow enough rsp objects, close channel");

                goto error;
            }
        }

        /* actual handling */
        process_request(rsp, req);

        klog_write(req, rsp);

        /* writing results */
        if (req->noreply) { /* noreply means no writing to buffers */
            request_reset(req);
            goto done;
        }

        nr = rsp;
        if (req->type == REQ_GET || req->type == REQ_GETS) {
            i = req->nfound;
            /* process returns an extra rsp which accounts for RSP_END */
            while (i > 0) {
                n = compose_rsp(&s->wbuf, nr);
                if (n < 0) {
                    log_warn("composing rsp erred, terminate channel");

                    goto error;
                }
                nr = STAILQ_NEXT(nr, next);
                i--;
            }
        }
        n = compose_rsp(&s->wbuf, nr);
        if (n < 0) {
            log_debug("composing rsp erred, terminate channel");

            goto error;
        }

        /* clean up resources */
        request_reset(req);
        for (i = 0; i < card; i++) {
            nr = STAILQ_NEXT(rsp, next);
            response_return(&rsp);
            rsp = nr;
        }
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
    s->ch->state = CHANNEL_TERM;
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
        if (s->ch->state == CHANNEL_TERM) {
            request_return((struct request **)&s->data);
            worker_close(s);
        }
    }
}
