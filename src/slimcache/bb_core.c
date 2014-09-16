#include <slimcache/bb_core.h>

#include <slimcache/bb_process.h>

#include <cc_debug.h>
#include <cc_event.h>

static void
_post_read(struct stream *stream, size_t nbyte)
{
    rstatus_t status;
    uint8_t *rpos;
    struct request *req;

    ASSERT(stream != NULL);

    //stats_thread_incr_by(data_read, nbyte);

    req = request_get();

    if (req == NULL) {
        log_error("cannot acquire request: OOM");
        status = compose_rsp_msg(stream->wbuf, RSP_SERVER_ERROR, false);
        if (status != CC_OK) {
            log_error("failed to send server error, status: %d", status);
        }
        /* raise an error event here */
        return;
    }

    while (mbuf_rsize(stream->rbuf) > 0) {
        rpos = stream->rbuf->rpos;
        request_reset(req);

        /* parsing */
        status = parse_req_hdr(req, stream->rbuf);
        if (status == CC_UNFIN) {
            stream->rbuf->rpos = rpos; /* abort/roll back incomplete requests */

            goto done;
        }
        if (status != CC_OK) { /* parsing errors are all client errors */
            log_warn("illegal request received, status: %d", status);

            status = compose_rsp_msg(stream->wbuf, RSP_CLIENT_ERROR, false);
            if (status != CC_OK) {
                log_error("failed to send client error, status: %d", status);
            }
            /* raise an error event here (to close the connection)
             * Note that we can also swallow the rest of the data and move
             * on, but let's keep it simple for now
             */

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
            /* raise an error event here (to close the connection) */

            goto done;
        }
    }

    /* raise write event */

done:
    request_put(req);
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
