#include <slimcache/bb_core.h>

#include <slimcache/bb_process.h>

#include <cc_debug.h>
#include <cc_event.h>
#include <cc_nio.h>

static void
core_error(struct stream *stream)
{
    log_debug(LOG_INFO, "close channel %p", stream->channel);
    struct conn *c = NULL;

    /* delete event on fd */
    switch (stream->type) {
    case CHANNEL_TCP:
        c = stream->channel;
        c->handler->close(c);

        break;

    default:
        NOT_REACHED();
    }
}

static void
_post_read(struct stream *stream, size_t nbyte)
{
    rstatus_t status;
    struct request *req;

    ASSERT(stream != NULL);

    //stats_thread_incr_by(data_read, nbyte);

    if (stream->data != NULL) {
        req = stream->data;
    } else {
        req = request_get();
        stream->data = req;
    }

    if (req == NULL) {
        log_error("cannot acquire request: OOM");
        status = compose_rsp_msg(stream->wbuf, RSP_SERVER_ERROR, false);
        if (status != CC_OK) {
            //stream->err = status;

            log_error("failed to send server error, status: %d", status);
        }

        goto done;
    }

    if (req->swallow) {
        status = parse_swallow(stream->rbuf);
        if (status == CC_OK) {
            request_reset(req);
        } else { /* CC_UNFIN */
            return;
        }
    }

    while (mbuf_rsize(stream->rbuf) > 0) {
        /* parsing */
        status = parse_req(req, stream->rbuf);
        if (status == CC_UNFIN) {
            goto done;
        }

        if (status != CC_OK) { /* parsing errors are all client errors */
            log_warn("illegal request received, status: %d", status);

            status = compose_rsp_msg(stream->wbuf, RSP_CLIENT_ERROR, false);
            if (status != CC_OK) {
                log_error("failed to send client error, status: %d", status);
            }

            goto done;
        }

        /* processing */
        status = process_request(req, stream->wbuf);
        if (status != CC_OK) {
            log_error("process request failed: %d", status);

            status = compose_rsp_msg(stream->wbuf, RSP_SERVER_ERROR, false);
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

    if (!req->swallow) {
        request_put(req);
        stream->data = NULL;
    }

done:
    return;
    /* raise write event if there's data to write */
}

rstatus_t
core_read(struct stream *stream)
{
    ASSERT(stream != NULL);
    ASSERT(stream->wbuf != NULL && stream->rbuf != NULL);

    uint32_t limit = mbuf_wsize(stream->rbuf);
    rstatus_t status;

    status = stream_read(stream, limit);

    return status;
}

static void
_post_write(struct stream *stream, size_t nbyte)
{
    ASSERT(stream != NULL);
    ASSERT(mbuf_rsize(stream->wbuf) >= nbyte);

    //stats_thread_incr_by(data_written, nbyte);

    stream->wbuf->rpos += nbyte;

    /* left-shift rbuf and wbuf */
    mbuf_lshift(stream->rbuf);
    mbuf_lshift(stream->wbuf);
}

rstatus_t
core_write(struct stream *stream)
{
    ASSERT(stream != NULL);
    ASSERT(stream->wbuf != NULL && stream->rbuf != NULL);

    uint32_t limit = mbuf_rsize(stream->wbuf);
    rstatus_t status;

    status = stream_write(stream, limit);

    return status;
}

static void
core_event(struct stream *stream, uint32_t events)
{
    rstatus_t status;

    log_debug(LOG_VERB, "event %04"PRIX32" on stream %d", events, stream);

    if (events & EVENT_ERR) {
        core_error(stream);
    }

    if (events & EVENT_READ) {
        status = core_read(stream);
        if (status == CC_ERETRY) { /* retry read */
            /* add read event */
        }
        if (status == CC_ERROR) {
            /* close channel */
        }
    }

    if (events & EVENT_WRITE) {
        status = core_write(stream);
        if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
            /* add write event */
        }
        if (status == CC_ERROR) {
            /* close channel */
        }
    }
}

