#include "debug.h"

#include "core/context.h"

#include "protocol/admin/admin_include.h"
#include "util/util.h"

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
#include <sysexits.h>

#define DEBUG_MODULE_NAME "core::debug"

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

static struct addrinfo *debug_ai;
static struct buf_sock *debug_sock;

static struct request req;
static struct response rsp;

static inline void
_debug_close(struct buf_sock *s)
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

    s = buf_sock_create(); /* debug thread: always directly create not borrow */
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
_debug_write(struct buf_sock *s)
{
    rstatus_i status;

    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    status = buf_tcp_write(s);

    return status;
}

static inline void
_debug_post_write(struct buf_sock *s)
{
    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);

    dbuf_shrink(&(s->rbuf));
    dbuf_shrink(&(s->wbuf));
}

static inline void
_debug_event_write(struct buf_sock *s)
{
    rstatus_i status;
    struct tcp_conn *c = s->ch;

    status = _debug_write(s);
    if (status == CC_ERETRY || status == CC_EAGAIN) {
        event_add_write(ctx->evb, hdl->wid(c), s);
    } else if (status == CC_ERROR) {
        c->state = CHANNEL_TERM;
    }
    _debug_post_write(s);
}

static inline void
_debug_read(struct buf_sock *s)
{
    ASSERT(s != NULL);
    ASSERT(s->wbuf != NULL && s->rbuf != NULL);

    dbuf_tcp_read(s);
}

static void
_debug_post_read(struct buf_sock *s)
{
    parse_rstatus_e status;

    admin_request_reset(&req);

    while (buf_rsize(s->rbuf) > 0) {
        int n;

        status = debug_parse_req(&req, s->rbuf);
        if (status == PARSE_EUNFIN) {
            goto done;
        }
        if (status != PARSE_OK) {
            log_info("illegal request received on debug port status %d",
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
        _debug_event_write(s);
    }
    return;

error:
    s->ch->state = CHANNEL_TERM;
}

static void
_debug_event_read(struct buf_sock *s)
{
    struct tcp_conn *c = s->ch;

    if (c->level == CHANNEL_META) {
        _tcp_accept(s);
    } else if (c->level == CHANNEL_BASE) {
        _debug_read(s);
        _debug_post_read(s);
    } else {
        NOT_REACHED();
    }
}

static void
_debug_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;

    if (events & EVENT_READ) {
        _debug_event_read(s);
    } else if (events & EVENT_WRITE) {
        _debug_event_write(s);
    } else if (events & EVENT_ERR) {
        s->ch->state = CHANNEL_TERM;
    } else {
        NOT_REACHED();
    }

    if (s->ch->state == CHANNEL_TERM || s->ch->state == CHANNEL_ERROR) {
        _debug_close(s);
    }
}

void
core_debug_setup(core_debug_options_st *options)
{
    struct tcp_conn *c;
    char *host = DEBUG_HOST;
    char *port = DEBUG_PORT;
    int timeout = DEBUG_TIMEOUT;
    int nevent = DEBUG_NEVENT;

    log_info("set up the %s module", DEBUG_MODULE_NAME);

    if (debug_init) {
        log_warn("debug has already been setup, re-creating");
        core_debug_teardown();
    }

    if (options != NULL) {
        host = option_str(&options->debug_host);
        port = option_str(&options->debug_port);
        timeout = option_uint(&options->debug_timeout);
        nevent = option_uint(&options->debug_nevent);
    }

    ctx->timeout = timeout;
    ctx->evb = event_base_create(nevent, _debug_event);
    if (ctx->evb == NULL) {
        log_crit("failed to set up debug thread; could not create event "
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

    debug_sock = buf_sock_create();
    if (debug_sock == NULL) {
        log_crit("failed to set up debug thread; could not get buf_sock");
        goto error;
    }

    debug_sock->hdl = hdl;

    if (CC_OK != getaddr(&debug_ai, host, port)) {
        log_crit("failed to resolve address for debug host & port");
        goto error;
    }
    c = debug_sock->ch;
    if (!hdl->open(debug_ai, c)) {
        log_crit("debug connection setup failed");
        goto error;
    }
    c->level = CHANNEL_META;
    event_add_read(ctx->evb, hdl->rid(c), debug_sock);

    debug_init = true;

    return;

error:
    core_debug_teardown();
    exit(EX_CONFIG);
}

void
core_debug_teardown(void)
{
    log_info("tear down the %s module", DEBUG_MODULE_NAME);

    if (!debug_init) {
        log_warn("%s has never been setup", DEBUG_MODULE_NAME);
    } else {
        event_base_destroy(&(ctx->evb));
        freeaddrinfo(debug_ai);
        buf_sock_destroy(&debug_sock);
    }
    debug_init = false;
}

static rstatus_i
_debug_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    return CC_OK;
}

void *
core_debug_evloop(void *arg)
{
    for(;;) {
        if (_debug_evwait() != CC_OK) {
            log_crit("debug loop exited due to failure");
            break;
        }
    }

    exit(1);
}
