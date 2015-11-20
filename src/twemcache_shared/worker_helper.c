#include <protocol/memcache_include.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

void
post_read(struct buf_sock *s)
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
        worker_event_write(s);
    }

    return;

error:
    request_return(&req);
    response_return_all(&rsp);
    s->ch->state = CHANNEL_TERM;
}
