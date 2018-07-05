#include "admin.h"

#include "core/context.h"

#include "protocol/admin/admin_include.h"
#include "util/util.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_event.h>
#include <channel/cc_channel.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>
#include <time/cc_timer.h>
#include <time/cc_wheel.h>

#include <netdb.h>
#include <stdbool.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sysexits.h>

#define ADMIN_MODULE_NAME "core::admin"

struct timing_wheel *tw;

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

static struct addrinfo *admin_ai;
static struct buf_sock *admin_sock;

static struct request req;
static struct response rsp;

static inline void
_admin_close(struct buf_sock *s)
{
    event_del(ctx->evb, hdl->rid(s->ch));
    hdl->term(s->ch);
    buf_sock_destroy(&s);
}

static inline void
_tcp_accept(struct buf_sock *ss)
{
    struct buf_sock *s;
    struct tcp_conn *sc = ss->ch;

    s = buf_sock_create(); /* admin thread: always directly create not borrow */
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

static inline void
_admin_event_write(struct buf_sock *s)
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
_admin_post_read(struct buf_sock *s)
{
    parse_rstatus_t status;

    admin_request_reset(&req);

    while (buf_rsize(s->rbuf) > 0) {
        int n;

        status = admin_parse_req(&req, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }
        if (status != PARSE_OK) {
            log_info("illegal request received on admin port status %d",
                     status);
            goto error;
        }

        /* processing */
        if (req.type == REQ_QUIT) {
            log_info("peer called quit");
            s->ch->state = CHANNEL_TERM;
            goto done;
        }

        admin_response_reset(&rsp);

        admin_process_request(&rsp, &req);

        n = admin_compose_rsp(&s->wbuf, &rsp);
        if (n < 0) {
            log_error("compose response error");
            goto error;
        }
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

    if (events & EVENT_READ) {
        _admin_event_read(s);
    } else if (events & EVENT_WRITE) {
        _admin_event_write(s);
    } else if (events & EVENT_ERR) {
        s->ch->state = CHANNEL_TERM;
    } else {
        NOT_REACHED();
    }

    if (s->ch->state == CHANNEL_TERM || s->ch->state == CHANNEL_ERROR) {
        _admin_close(s);
    }
}

void
core_admin_setup(admin_options_st *options)
{
    struct tcp_conn *c;
    struct timeout tick;
    char *host = ADMIN_HOST;
    char *port = ADMIN_PORT;
    int timeout = ADMIN_TIMEOUT;
    int nevent = ADMIN_NEVENT;
    uint64_t tick_ms = ADMIN_TW_TICK;
    size_t cap = ADMIN_TW_CAP;
    size_t ntick = ADMIN_TW_NTICK;

    log_info("set up the %s module", ADMIN_MODULE_NAME);

    if (admin_init) {
        log_warn("admin has already been setup, re-creating");
        core_admin_teardown();
    }

    if (options != NULL) {
        host = option_str(&options->admin_host);
        port = option_str(&options->admin_port);
        timeout = option_uint(&options->admin_timeout);
        nevent = option_uint(&options->admin_nevent);
        tick_ms = option_uint(&options->admin_tw_tick);
        cap = option_uint(&options->admin_tw_cap);
        ntick = option_uint(&options->admin_tw_ntick);
    }

    ctx->timeout = timeout;
    ctx->evb = event_base_create(nevent, _admin_event);
    if (ctx->evb == NULL) {
        log_crit("failed to set up admin thread; could not create event "
                 "base for control plane");
        goto error;
    }

    hdl->accept = (channel_accept_fn)tcp_accept;
    hdl->reject = (channel_reject_fn)tcp_reject;
    hdl->open = (channel_open_fn)tcp_listen;
    hdl->term = (channel_term_fn)tcp_close;
    hdl->recv = (channel_recv_fn)tcp_recv;
    hdl->send = (channel_send_fn)tcp_send;
    hdl->rid = (channel_id_fn)tcp_read_id;
    hdl->wid = (channel_id_fn)tcp_write_id;

    admin_sock = buf_sock_create();
    if (admin_sock == NULL) {
        log_crit("failed to set up admin thread; could not get buf_sock");
        goto error;
    }

    admin_sock->hdl = hdl;

    if (CC_OK != getaddr(&admin_ai, host, port)) {
        log_crit("failed to resolve address for admin host & port");
        goto error;
    }
    c = admin_sock->ch;
    if (!hdl->open(admin_ai, c)) {
        log_crit("admin connection setup failed");
        goto error;
    }
    c->level = CHANNEL_META;
    event_add_read(ctx->evb, hdl->rid(c), admin_sock);

    timeout_set_ms(&tick, tick_ms);
    tw = timing_wheel_create(&tick, cap, ntick);
    if (tw == NULL) {
        log_crit("create timing wheel failed");
        goto error;
    }
    timing_wheel_start(tw);

    admin_init = true;

    return;

error:
    core_admin_teardown();
    exit(EX_CONFIG);
}

void
core_admin_teardown(void)
{
    log_info("tear down the %s module", ADMIN_MODULE_NAME);

    if (!admin_init) {
        log_warn("%s has never been setup", ADMIN_MODULE_NAME);
    } else {
        timing_wheel_stop(tw);
        timing_wheel_destroy(&tw);
        event_base_destroy(&(ctx->evb));
        freeaddrinfo(admin_ai);
        buf_sock_destroy(&admin_sock);
    }
    admin_init = false;
}

struct timeout_event *
core_admin_register(uint64_t intvl_ms, timeout_cb_fn cb, void *arg)
{
    struct timeout delay;

    ASSERT(admin_init);

    timeout_set_ms(&delay, intvl_ms);
    return timing_wheel_insert(tw, &delay, true, cb, arg);
}

static rstatus_i
_admin_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    return CC_OK;
}

void
core_admin_evloop(void)
{
    for(;;) {
        if (_admin_evwait() != CC_OK) {
            log_crit("admin loop exited due to failure");
            break;
        }

        timing_wheel_execute(tw);
    }

    exit(1);
}
