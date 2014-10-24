#include <slimcache/bb_core.h>

#include <slimcache/bb_process.h>

#include <cc_debug.h>
#include <cc_event.h>
#include <cc_nio.h>
#include <cc_stream.h>

static struct context {
    struct event_base  *evb;
    int                timeout;
} context;

static struct context *ctx = &context;
static struct stream *ss; /* server stream */
static struct conn *sc; /* server connection */
static stream_handler_t server_hdl;
static stream_handler_t conn_hdl;


static void
core_close(struct stream *stream)
{
    log_verb("core close on stream %p", stream);

    if (stream->owner != ctx) { /* not owned by this event loop anymore */
        return;
    }

    log_info("close channel %p", stream->channel);

    event_deregister(ctx->evb, stream->handler->fd(stream->channel));
    stream->handler->close(stream->channel);
    stream->owner = NULL;
    stream_return(stream);
}

static void
_post_read(struct stream *stream, size_t nbyte)
{
    rstatus_t status;
    struct request *req;

    log_verb("post read on stream %p after writing %zu bytes", stream, nbyte);

    ASSERT(stream != NULL);

    //stats_thread_incr_by(data_read, nbyte);

    if (stream->data != NULL) {
        req = stream->data;
    } else {
        req = request_borrow();
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
        log_verb("%"PRIu32" bytes left", mbuf_rsize(stream->rbuf));

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
        log_verb("wbuf free: %"PRIu32" B", mbuf_wsize(stream->wbuf));
        status = process_request(req, stream->wbuf);
        log_verb("wbuf free: %"PRIu32" B", mbuf_wsize(stream->wbuf));

        if (status == CC_ERDHUP) {
            log_info("peer called quit");
            if (stream->type == CHANNEL_TCP) {
                ((struct conn *)stream->channel)->state = CONN_EOF;
            } else {
                log_error("unsupported or unknown channel type");
            }
            return;
        }

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
        request_return(req);
        stream->data = NULL;
    }

done:
    /* TODO: call stream write directly, use events only for retries */
    if (mbuf_rsize(stream->wbuf) > 0) {
        event_add_write(ctx->evb, stream->handler->fd(stream->channel), stream);
    }
    return;
}

static rstatus_t
core_read(struct stream *stream)
{
    log_verb("core read on stream %p", stream);

    ASSERT(stream != NULL);
    ASSERT(stream->wbuf != NULL && stream->rbuf != NULL);

    uint32_t limit = mbuf_wsize(stream->rbuf);
    rstatus_t status;

    /* TODO(yao): refactor this after stream refactoring */
    if (limit == 0) {
        struct mbuf *buf = stream->rbuf;
        log_info("read buffer full: start %p, rpos %p, wpos %p, end %p",
                buf->start, buf->rpos, buf->wpos, buf->end);
    }

    status = stream_read(stream, limit);

    return status;
}

static void
_post_write(struct stream *stream, size_t nbyte)
{
    log_verb("post write on stream %p after writing %zu bytes", stream, nbyte);

    ASSERT(stream != NULL);

    //stats_thread_incr_by(data_written, nbyte);

    /* left-shift rbuf and wbuf */
    mbuf_lshift(stream->rbuf);
    mbuf_lshift(stream->wbuf);
}

static rstatus_t
core_write(struct stream *stream)
{
    log_verb("core write on stream %p", stream);

    ASSERT(stream != NULL);
    ASSERT(stream->wbuf != NULL && stream->rbuf != NULL);

    uint32_t limit = mbuf_rsize(stream->wbuf);
    rstatus_t status;

    status = stream_write(stream, limit);

    return status;
}

/* TCP only, nbyte is not used */
static void
core_listen(struct stream *stream, size_t nbyte)
{
    struct stream *s;
    struct conn *c;

    c = server_accept(stream->channel);
    if (c == NULL) {
        log_error("connection establishment failed: cannot accept");

        return;
    }

    s = stream_borrow();
    if (s == NULL) {
        log_error("connection establishment failed: cannot alloc stream");
        server_close(c);

        return;
    }

    s->owner = ctx;
    s->type = CHANNEL_TCP;
    s->channel = c;
    s->err = 0;
    s->handler = &conn_hdl;
    s->data = NULL;
    event_register(ctx->evb, c->sd, s);
}

static bool
_should_close(struct stream *s) {
    if (s->type == CHANNEL_TCP) {
        struct conn *c = s->channel;
        return (c->state == CONN_EOF || c->state == CONN_CLOSE);
    } else {
        log_error("unsupported or unknown channel type");
        return false;
    }
}

static void
core_event(void *arg, uint32_t events)
{
    rstatus_t status;
    struct stream *stream = arg;

    log_verb("event %06"PRIX32" on stream %p", events, stream);

    if (events & EVENT_ERR) {
        core_close(stream);

        return;
    }

    if (events & EVENT_READ) {
        if (stream->type == CHANNEL_TCPLISTEN) {
            core_listen(stream, 0);

            return;
        }

        status = core_read(stream);
        if (status == CC_ERETRY) { /* retry read */
            event_add_read(ctx->evb, stream->handler->fd(stream->channel),
                    stream);
        } else if (status == CC_ERROR) {
            core_close(stream);

            return;
        }
    }

    if (events & EVENT_WRITE) {
        status = core_write(stream);
        if (status == CC_ERETRY || status == CC_EAGAIN) { /* retry write */
            event_add_write(ctx->evb, stream->handler->fd(stream->channel),
                    stream);

            return;
        }
        if (status == CC_ERROR) {
            core_close(stream);

            return;
        }
    }

    if (_should_close(stream)) { /* closing _after_ all writes are completed */
        core_close(stream);

        return;
    }
}

static void
handler_setup(void)
{
    server_hdl.open = NULL; /* created during setup */
    server_hdl.close = (channel_close_t)server_close;
    server_hdl.fd = (channel_fd_t)conn_fd;
    server_hdl.pre_read = NULL;
    server_hdl.post_read = core_listen;
    server_hdl.pre_write = NULL; /* server connection doesn't write */
    server_hdl.post_write = NULL;

    conn_hdl.open = NULL; /* created by server_hdl.post_read */
    conn_hdl.close = (channel_close_t)conn_close;
    conn_hdl.fd = (channel_fd_t)conn_fd;
    conn_hdl.pre_read = NULL;
    conn_hdl.post_read = _post_read;
    conn_hdl.pre_write = NULL;
    conn_hdl.post_write = _post_write;
}

rstatus_t
core_setup(struct addrinfo *ai)
{
    handler_setup();

    ctx->timeout = 100;
    ctx->evb = event_base_create(1024, core_event);
    if (ctx->evb == NULL) {
        return CC_ERROR;
    }

    sc = server_listen(ai);
    if (sc == NULL) {
        log_error("server connection setup failed");

        return CC_ERROR;
    }

    ss = stream_borrow();
    if (ss == NULL) {
        log_error("cannot get server stream: OOM");

        return CC_ERROR;
    }
    ss->owner = ctx;
    ss->type = CHANNEL_TCPLISTEN;
    ss->channel = sc;
    ss->err = 0;
    ss->handler = &server_hdl;
    ss->data = NULL;
    event_register(ctx->evb, sc->sd, ss);

    return CC_OK;
}

void
core_teardown(void)
{
    stream_return(ss);
    event_base_destroy(ctx->evb);
}

rstatus_t
core_evwait(void)
{
    int n;
    n = event_wait(ctx->evb, ctx->timeout);

    if (n < 0) {
        return n;
    }

    return CC_OK;
}
