#include <core/background.h>

#include <core/shared.h>

#include <protocol/admin_include.h>
#include <time/time.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_event.h>
#include <channel/cc_channel.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

#include <netdb.h>
#include <stdbool.h>
#include <sys/socket.h>
#include <sys/types.h>

#define BACKGROUND_MODULE_NAME "core::background"

static bool background_init = false;

static struct context context;
static struct context *ctx = &context;

static channel_handler_t handlers;
static channel_handler_t *hdl = &handlers;

static struct buf_sock *serversock;

static inline void
_admin_close(struct buf_sock *s)
{
    event_deregister(ctx->evb, s->ch->sd);
    hdl->term(s->ch);
    buf_sock_return(&s);
}

static inline void
_tcp_accept(struct buf_sock *ss)
{
    struct buf_sock *s;
    struct tcp_conn *sc = ss->ch;

    s = buf_sock_borrow();
    if (s == NULL) {
        log_error("establish connection failed: cannot allocate buf_sock, "
                "reject connection request");
        ss->hdl->reject(sc); /* server rejects connection by closing it */
        return;
    }

    if (!ss->hdl->accept(sc, s->ch)) {
        return;
    }

    s->owner = ctx;
    s->hdl = hdl;

    event_add_read(ctx->evb, hdl->rid(s->ch), s);
}

static inline rstatus_t
_admin_write(struct buf_sock *s)
{
    rstatus_t status;

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    status = buf_tcp_write(s);

    return status;
}

static inline void
_admin_post_write(struct buf_sock *s)
{
    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);

    dbuf_shrink(&(s->rbuf));
    dbuf_shrink(&(s->wbuf));
}

static inline void
_admin_event_write(struct buf_sock *s)
{
    rstatus_t status;
    struct tcp_conn *c = s->ch;

    status = _admin_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) {
        event_add_write(ctx->evb, hdl->wid(c), s);
    } else if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
    }
    _admin_post_write(s);
}

static inline void
_admin_read(struct buf_sock *s)
{
    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    dbuf_tcp_read(s);
}

static void
_admin_post_read(struct buf_sock *s)
{
    parse_rstatus_t status;
    struct op *op;

    if (s->data == NULL) {
        s->data = op_create();
    }

    op = s->data;

    if (op == NULL) {
        goto error;
    }

    while (buf_rsize(s->rbuf) > 0) {
        struct reply *rep;
        int n;

        status = parse_op(op, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }

        if (status != PARSE_OK) {
            log_info("illegal request received on admin port status %d",
                     status);
            goto error;
        }

        /* processing */
        if (op->type == OP_QUIT) {
            log_info("peer called quit");
            s->ch->state = CHANNEL_TERM;
            goto done;
        }

        /* no chained replies for now */
        rep = reply_create();
        if (rep == NULL) {
            log_error("could not allocate reply object");
            goto error;
        }

        process_op(rep, op);

        n = compose_rep(&s->wbuf, rep);
        if (n < 0) {
            log_error("compose reply error");
            reply_destroy(&rep);
            goto error;
        }

        op_reset(op);
        reply_destroy(&rep);
    }

done:
    if (buf_rsize(s->wbuf) > 0) {
        _admin_event_write(s);
    }
    return;

error:
    s->ch->state = CHANNEL_TERM;
}

static void
_admin_event_read(struct buf_sock *s)
{
    struct tcp_conn *c = s->ch;

    if (c->level == CHANNEL_META) {
        _tcp_accept(s);
    } else if (c->level == CHANNEL_BASE) {
        _admin_read(s);
        _admin_post_read(s);
    } else {
        NOT_REACHED();
    }
}

static void
_admin_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;

    if (events & EVENT_ERR) {
        _admin_close(s);
        return;
    }

    if (events & EVENT_READ) {
        _admin_event_read(s);
    }

    if (events & EVENT_WRITE) {
        _admin_event_write(s);
    }
}

rstatus_t
background_setup(struct addrinfo *ai, int tick)
{
    struct tcp_conn *c;

    log_info("set up the %s module", BACKGROUND_MODULE_NAME);

    if (background_init) {
        log_error("background has already been setup, aborting");
        return CC_ERROR;
    }

    ctx->timeout = tick;
    ctx->evb = event_base_create(1024, _admin_event);
    if (ctx->evb == NULL) {
        log_crit("failed to set up background thread; could not create event "
                 "base for control plane");
        return CC_ERROR;
    }

    hdl->accept = (bool (*)(channel_t, channel_t))tcp_accept;
    hdl->reject = (void (*)(channel_t))tcp_reject;
    hdl->open = (bool (*)(address_t, channel_t))tcp_listen;
    hdl->term = (void (*)(channel_t))tcp_close;
    hdl->recv = (ssize_t (*)(channel_t, void *, size_t))tcp_recv;
    hdl->send = (ssize_t (*)(channel_t, void *, size_t))tcp_send;
    hdl->rid = (ch_id_t (*)(channel_t))tcp_read_id;
    hdl->wid = (ch_id_t (*)(channel_t))tcp_write_id;

    serversock = buf_sock_borrow();
    if (serversock == NULL) {
        log_crit("failed to set up background thread; could not get buf_sock");
        return CC_ERROR;
    }

    serversock->hdl = hdl;

    c = serversock->ch;
    if (!hdl->open(ai, c)) {
        log_crit("admin connection setup failed");
        return CC_ERROR;
    }
    c->level = CHANNEL_META;

    event_add_read(ctx->evb, hdl->rid(c), serversock);
    background_init = true;

    return CC_OK;
}

void
background_teardown(void)
{
    log_info("tear down the %s module", BACKGROUND_MODULE_NAME);

    if (!background_init) {
        log_warn("%s has never been setup", BACKGROUND_MODULE_NAME);
    } else {
        buf_sock_return(&serversock);
        event_base_destroy(&(ctx->evb));
    }
    background_init = false;
}

static rstatus_t
background_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    time_update();

    return CC_OK;
}

void *
background_evloop(void *arg)
{
    rstatus_t status;

    for(;;) {
        status = background_evwait();
        if (status != CC_OK) {
            log_crit("background loop exited due to failure");
            break;
        }

        /* time_wheel execute called here */
    }

    exit(1);
}
