#include <util/bb_core_server.h>

#include <bb_stats.h>
#include <time/bb_time.h>
#include <util/bb_core_shared.h>

#include <cc_channel.h>
#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

#include <errno.h>
#include <string.h>

static struct context context;
static struct context *ctx = &context;

static channel_handler_t handlers;
static channel_handler_t *hdl = &handlers;

static struct buf_sock *serversock; /* server buf_sock */

static void
_server_close(struct buf_sock *s)
{
    log_info("core close on buf_sock %p", s);

    event_deregister(ctx->evb, s->ch->sd);

    hdl->term(s->ch);
    request_return((struct request **)&s->data);
    buf_sock_return(&s);
}

static void
_server_pipe_write(void)
{
    /* TODO(kyang): when pipe channel is ready, replace this with the new facilities */
    /* write to conn_fds[1] to send event to worker thread */
    if (write(conn_fds[1], "", 1) != 0) {
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            /* Not sure if this is how this should be done, please check */
            log_verb("server core: write on pipe would block, retry");
            event_add_write(ctx->evb, conn_fds[1], NULL);
        } else {
            /* Other error while writing */
            log_error("could not write to conn_fds pipe, %s", strerror(errno));
        }
    }
}

static void
_tcp_accept(struct buf_sock *ss)
{
    struct buf_sock *s;
    struct conn *sc = ss->ch;

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

    /* push buf_sock to queue */
    ring_array_push(&s, conn_arr);

    _server_pipe_write();
}

static void
_server_event_read(struct buf_sock *s)
{
    struct conn *c = s->ch;

    if (c->level == CHANNEL_META) {
        _tcp_accept(s);
    } else {
        NOT_REACHED();
    }
}

static void
core_server_event(void *arg, uint32_t events)
{
    struct buf_sock *s = arg;

    log_verb("server event %06"PRIX32" on buf_sock %p", events, s);

    if (events & EVENT_ERR) {
        INCR(server_event_error);
        _server_close(s);

        return;
    }

    if (events & EVENT_READ) {
        log_verb("processing server read event on buf_sock %p", s);

        INCR(server_event_read);
        _server_event_read(s);
    }

    if (events & EVENT_WRITE) {
        log_verb("processing server write event");
        _server_pipe_write();

        INCR(server_event_write);
    }
}

rstatus_t
core_server_setup(struct addrinfo *ai)
{
    struct conn *c;

    ctx->timeout = 100;
    ctx->evb = event_base_create(1024, core_server_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup server core; could not create event_base");
        return CC_ERROR;
    }

    hdl->accept = tcp_accept;
    hdl->reject = tcp_reject;
    hdl->open = tcp_listen;
    hdl->term = tcp_close;
    hdl->recv = tcp_recv;
    hdl->send = tcp_send;
    hdl->id = conn_id;

    /**
     * Here we give server socket a buf_sock purely because it is difficult to
     * write code in the core event loop that would accommodate different types
     * of structs at the moment. However, this doesn't have to be the case in
     * the future. We can choose to wrap different types in a common header-
     * one that contains a type field and a pointer to the actual struct, or
     * define common fields, like how posix sockaddr structs are used.
     */
    serversock = buf_sock_borrow();
    if (serversock == NULL) {
        log_crit("failed to setup server core; could not get buf_sock");
        return CC_ERROR;
    }

    serversock->hdl = hdl;

    c = serversock->ch;
    if (!hdl->open(ai, c)) {
        log_error("server connection setup failed");
        return CC_ERROR;
    }
    c->level = CHANNEL_META;

    event_add_read(ctx->evb, hdl->id(c), serversock);

    return CC_OK;
}

void
core_server_teardown(void)
{
    buf_sock_return(&serversock);
    event_base_destroy(&(ctx->evb));
}

static rstatus_t
core_server_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(server_event_loop);
    INCR_N(server_event_total, n);
    time_update();

    return CC_OK;
}

void
core_server_evloop(void)
{
    rstatus_t status;

    for(;;) {
        status = core_server_evwait();
        if (status != CC_OK) {
            log_crit("server core event loop exited due to failure");
            break;
        }
    }
}
