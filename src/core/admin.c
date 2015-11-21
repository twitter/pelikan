#include <core/admin.h>

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

#define ADMIN_MODULE_NAME "core::admin"

static bool admin_init = false;

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

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

static inline rstatus_i
_admin_write(struct buf_sock *s)
{
    rstatus_i status;

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

void
admin_event_write(struct buf_sock *s)
{
    rstatus_i status;
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
_admin_event_read(struct buf_sock *s)
{
    struct tcp_conn *c = s->ch;

    if (c->level == CHANNEL_META) {
        _tcp_accept(s);
    } else if (c->level == CHANNEL_BASE) {
        _admin_read(s);
        admin_post_read(s);
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
        admin_event_write(s);
    }
}

rstatus_i
admin_setup(struct addrinfo *ai, int tick)
{
    struct tcp_conn *c;

    log_info("set up the %s module", ADMIN_MODULE_NAME);

    if (admin_init) {
        log_error("admin has already been setup, aborting");
        return CC_ERROR;
    }

    ctx->timeout = tick;
    ctx->evb = event_base_create(1024, _admin_event);
    if (ctx->evb == NULL) {
        log_crit("failed to set up admin thread; could not create event "
                 "base for control plane");
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

    serversock = buf_sock_borrow();
    if (serversock == NULL) {
        log_crit("failed to set up admin thread; could not get buf_sock");
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
    admin_init = true;

    return CC_OK;
}

void
admin_teardown(void)
{
    log_info("tear down the %s module", ADMIN_MODULE_NAME);

    if (!admin_init) {
        log_warn("%s has never been setup", ADMIN_MODULE_NAME);
    } else {
        buf_sock_return(&serversock);
        event_base_destroy(&(ctx->evb));
    }
    admin_init = false;
}

static rstatus_i
admin_evwait(void)
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
admin_evloop(void *arg)
{
    rstatus_i status;

    for(;;) {
        status = admin_evwait();
        if (status != CC_OK) {
            log_crit("admin loop exited due to failure");
            break;
        }

        /* time_wheel execute called here */
    }

    exit(1);
}
