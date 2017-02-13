#include "server.h"

#include "core/context.h"
#include "shared.h"

#include <time/time.h>
#include <util/util.h>

#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_channel.h>
#include <channel/cc_pipe.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

#include <errno.h>
#include <string.h>
#include <sysexits.h>

#define SERVER_MODULE_NAME "core::server"

static bool server_init = false;
static server_metrics_st *server_metrics = NULL;

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

static struct addrinfo *server_ai;
static struct buf_sock *server_sock; /* server buf_sock */

static inline void
_server_close(struct buf_sock *s)
{
    log_info("core close on buf_sock %p", s);

    event_del(ctx->evb, hdl->rid(s->ch));

    hdl->term(s->ch);
    buf_sock_return(&s);
}

static inline void
_server_pipe_write(void)
{
    ASSERT(pipe_c != NULL);

    ssize_t status = pipe_send(pipe_c, "", 1);

    if (status == 0 || status == CC_EAGAIN) {
        /* retry write */
        log_verb("server core: retry send on pipe");
        event_add_write(ctx->evb, pipe_write_id(pipe_c), NULL);
    } else if (status == CC_ERROR) {
        /* other reasn write can't be done */
        log_error("could not write to pipe - %s", strerror(pipe_c->err));
    }

    /* else, pipe write succeeded and no action needs to be taken */
}

/* returns true if a connection is present, false if no more pending */
static inline bool
_tcp_accept(struct buf_sock *ss)
{
    struct buf_sock *s;
    struct tcp_conn *sc = ss->ch;

    s = buf_sock_borrow();
    if (s == NULL) {
        log_error("establish connection failed: cannot allocate buf_sock, "
                "reject connection request");
        ss->hdl->reject(sc); /* server rejects connection by closing it */
        return true;
    }

    if (!ss->hdl->accept(sc, s->ch)) {
        buf_sock_return(&s);
        return false;
    }

    /* push buf_sock to queue */
    ring_array_push(&s, conn_arr);

    _server_pipe_write();

    return true;
}

static inline void
_server_event_read(struct buf_sock *s)
{
    struct tcp_conn *c = s->ch;

    ASSERT(c->level == CHANNEL_META);

    while (_tcp_accept(s));
}

static void
_server_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;

    log_verb("server event %06"PRIX32" on buf_sock %p", events, s);

    if (events & EVENT_ERR) {
        INCR(server_metrics, server_event_error);
        _server_close(s);

        return;
    }

    if (events & EVENT_READ) {
        log_verb("processing server read event on buf_sock %p", s);

        INCR(server_metrics, server_event_read);
        _server_event_read(s);
    }

    if (events & EVENT_WRITE) {
        /* the only server write event is write on pipe */

        log_verb("processing server write event");
        _server_pipe_write();

        INCR(server_metrics, server_event_write);
    }
}

void
core_server_setup(server_options_st *options, server_metrics_st *metrics)
{
    struct tcp_conn *c;
    char *host = SERVER_HOST;
    char *port = SERVER_PORT;
    int timeout = SERVER_TIMEOUT;
    int nevent = SERVER_NEVENT;

    log_info("set up the %s module", SERVER_MODULE_NAME);

    if (server_init) {
        log_warn("server has already been setup, re-creating");
        core_server_teardown();
    }

    server_metrics = metrics;

    if (options != NULL) {
        host = option_str(&options->server_host);
        port = option_str(&options->server_port);
        timeout = option_uint(&options->server_timeout);
        nevent = option_uint(&options->server_nevent);
    }

    ctx->timeout = timeout;
    ctx->evb = event_base_create(nevent, _server_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup server core; could not create event_base");
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

    /**
     * Here we give server socket a buf_sock purely because it is difficult to
     * write code in the core event loop that would accommodate different types
     * of structs at the moment. However, this doesn't have to be the case in
     * the future. We can choose to wrap different types in a common header-
     * one that contains a type field and a pointer to the actual struct, or
     * define common fields, like how posix sockaddr structs are used.
     */
    server_sock = buf_sock_borrow();
    if (server_sock == NULL) {
        log_crit("failed to setup server core; could not get buf_sock");
        goto error;
    }

    server_sock->hdl = hdl;
    if (CC_OK != getaddr(&server_ai, host, port)) {
        log_crit("failed to resolve address for admin host & port");
        goto error;
    }

    c = server_sock->ch;
    if (!hdl->open(server_ai, c)) {
        log_crit("server connection setup failed");
        goto error;
    }
    c->level = CHANNEL_META;

    event_add_read(ctx->evb, hdl->rid(c), server_sock);

    server_init = true;

    return;

error:
    core_server_teardown();
    exit(EX_CONFIG);
}

void
core_server_teardown(void)
{
    log_info("tear down the %s module", SERVER_MODULE_NAME);

    if (!server_init) {
        log_warn("%s has never been setup", SERVER_MODULE_NAME);
    } else {
        event_base_destroy(&(ctx->evb));
        freeaddrinfo(server_ai);
        buf_sock_return(&server_sock);
    }
    server_metrics = NULL;
    server_init = false;
}

static rstatus_i
_server_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(server_metrics, server_event_loop);
    INCR_N(server_metrics, server_event_total, n);
    time_update();

    return CC_OK;
}

void
core_server_evloop(void)
{
    for(;;) {
        if (_server_evwait() != CC_OK) {
            log_crit("server core event loop exited due to failure");
            break;
        }
    }
}
