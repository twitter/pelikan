#include "server.h"

#include "core/context.h"
#include "shared.h"

#include "util/util.h"

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

struct pipe_conn *pipe_new = NULL;   /* server(w) -> worker(r) */
struct pipe_conn *pipe_term = NULL;  /* worker(w) -> server(r) */
struct ring_array *conn_new = NULL;  /* server(w) -> worker(r) */
struct ring_array *conn_term = NULL; /* worker(w) -> server(r) */

static server_metrics_st *server_metrics = NULL;

static struct context context;
static struct context *ctx = &context;

static channel_handler_st handlers;
static channel_handler_st *hdl = &handlers;

static struct addrinfo *server_ai;
static struct buf_sock *server_sock; /* server buf_sock */

/* Note: server thread currently owns the stream (buf_sock) pool. Other threads
 * either need to get the connection from server (the case for worker thread) or
 * have to directly create their own, instead of borrowing (the case for admin
 * thread), to avoid concurrency issues around pooling operations, which are not
 * thread-safe.
 */

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
    ASSERT(pipe_new != NULL);

    ssize_t status = pipe_send(pipe_new, "", 1);

    if (status == 0 || status == CC_EAGAIN) {
        /* retry write */
        log_verb("server core: retry send on pipe");
        event_add_write(ctx->evb, pipe_write_id(pipe_new), NULL);
    } else if (status == CC_ERROR) {
        log_error("could not write to pipe - %s", strerror(pipe_new->err));
    }
}

/* pipe_read recycles returned streams from worker thread */
static inline void
_server_pipe_read(void)
{
    struct buf_sock *s;
    char buf[RING_ARRAY_DEFAULT_CAP]; /* buffer for discarding pipe data */
    int i;
    rstatus_i status;

    ASSERT(pipe_term != NULL);

    i = pipe_recv(pipe_term, buf, RING_ARRAY_DEFAULT_CAP);
    if (i < 0) { /* errors, do not read from ring array */
        log_warn("not reclaiming connections due to pipe error");
        return;
    }

    /* each byte in the pipe corresponds to a connection in the array */
    for (; i > 0; --i) {
        status = ring_array_pop(&s, conn_term);
        if (status != CC_OK) {
            log_warn("event number does not match conn queue: missing %d conns",
                    i);
            return;
        }
        log_verb("Recycling buf_sock %p from worker thread", s);
        hdl->term(s->ch);
        buf_sock_return(&s);
    }
}

/* returns true if a connection is present, false if no more pending */
static inline bool
_tcp_accept(struct buf_sock *ss)
{
    struct buf_sock *s;
    struct tcp_conn *sc = ss->ch;

    s = buf_sock_borrow();
    if (s == NULL) {
        /*
         * TODO: what's the best way to respond to DDoS?
         *
         * If the DDoS is intentional, the best response is probably to do as
         * little work as possible, and hope OS can handle/shed the load.
         *
         * If DDoS is caused by synchronized client connect attempts with
         * a reasonable backoff policy, we probably can close the connections
         * right away to trigger the client-side policy.
         *
         * If the client-side policy is for timeout only but not for other
         * errors, we probably want to wait (sleep()), so the client-side
         * backoff can be triggered.
         *
         * If the client-side logic does not have any backoff, we are pretty
         * much in the same situation as an intentional DDoS.
         */
        /*
         * Aside from properly handle the connections, another issue is what
         * server should do with its CPU time. There are three options:
         *   - keep handling incoming events (mostly rejecting connections)
         *   - sleep for a while and then wake up, hoping things change by then
         *   - stop handling incoming events until a connection is freed
         *
         * Delayed response saves CPU resources and generally makes more sense
         * for the server, knowing that client probably will retry and succeed
         * eventually. However at this point it is not clear to me whether it's
         * better to do a timed sleep or a conditional sleep.
         * Timed sleep is easy to implement but a little inflexible; conditional
         * sleep is the smartest option but requires cross-thread communication.
         *
         * Twemcache enables/disables event on the listening port dinamically,
         * but the handling is not really thread-safe.
         */
        log_error("establish connection failed: cannot allocate buf_sock, "
                "reject connection request");
        ss->hdl->reject(sc); /* server rejects connection by closing it */
        return false;
    }

    if (!ss->hdl->accept(sc, s->ch)) {
        buf_sock_return(&s);
        return false;
    }

    /* push buf_sock to queue */
    ring_array_push(&s, conn_new);
    /* notify worker, note this may fail and will be retried via write event */
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
    log_verb("server event %06"PRIX32" with data %p", events, s);

    if (s == NULL) { /* event on pipe */
        if (events & EVENT_READ) { /* terminating connection from worker */
            log_verb("processing server read event on pipe");
            INCR(server_metrics, server_event_read);
            _server_pipe_read();
        } else if (events & EVENT_WRITE) { /* retrying worker notification */
            log_verb("processing server write event on pipe");
            INCR(server_metrics, server_event_write);
            _server_pipe_write();
        } else { /* EVENT_ERR */
            log_debug("processing server error event on pipe");
            INCR(server_metrics, server_event_error);
        }
    } else { /* event on listening socket */
        if (events & EVENT_READ) {
            log_verb("processing server read event on buf_sock %p", s);
            INCR(server_metrics, server_event_read);
            _server_event_read(s);
        } else if (events & EVENT_ERR) { /* effectively refusing new conn */
            /* TODO: shall we retry bind and listen ? */
            log_debug("processing server error event on listening socket");
            _server_close(s);
        } else {
            NOT_REACHED();
        }
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

    /* setup shared data structures between server and worker */
    pipe_new = pipe_conn_create();
    pipe_term = pipe_conn_create();
    if (pipe_new == NULL || pipe_term == NULL) {
        log_error("Could not create connection for pipe, abort");
        goto error;
    }

    if (!pipe_open(NULL, pipe_new)) {
        log_error("Could not open pipe for new connection: %s",
                strerror(pipe_new->err));
        goto error;
    }
    if (!pipe_open(NULL, pipe_term)) {
        log_error("Could not open pipe for terminated connection: %s",
                strerror(pipe_term->err));
        goto error;
    }


    pipe_set_nonblocking(pipe_new);
    pipe_set_nonblocking(pipe_term);

    conn_new = ring_array_create(sizeof(struct buf_sock *),
            RING_ARRAY_DEFAULT_CAP);
    conn_term = ring_array_create(sizeof(struct buf_sock *),
            RING_ARRAY_DEFAULT_CAP);
    if (conn_new == NULL || conn_term == NULL) {
        log_error("core setup failed: could not allocate conn array(s)");
        goto error;
    }

    ctx->timeout = timeout;
    ctx->evb = event_base_create(nevent, _server_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup server core; could not create event_base");
        goto error;
    }

    hdl->accept = (channel_accept_fn)tcp_accept;
    hdl->reject = (channel_reject_fn)tcp_reject_all;
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
    event_add_read(ctx->evb, pipe_read_id(pipe_term), NULL);

    server_init = true;

    return;

error:
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
    ring_array_destroy(conn_term);
    ring_array_destroy(conn_new);
    pipe_conn_destroy(&pipe_term);
    pipe_conn_destroy(&pipe_new);
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

    return CC_OK;
}

void *
core_server_evloop(void *arg)
{
    for(;;) {
        if (_server_evwait() != CC_OK) {
            log_crit("server core event loop exited due to failure");
            break;
        }
    }

    exit(1);
}
