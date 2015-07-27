#include <core/worker.h>

#include <time/time.h>
#include <protocol/memcache/codec.h>
#include <core/shared.h>

/*
 * TODO(yao): this doesn't look clean, protocol, process shouldn't be assumed
 * in the event handling part, but rather should be passed in
 */

#include <cc_debug.h>
#include <cc_event.h>
#include <cc_ring_array.h>
#include <channel/cc_channel.h>
#include <channel/cc_pipe.h>
#include <channel/cc_tcp.h>

#include <stream/cc_sockio.h>

#define WORKER_MODULE_NAME "util::worker"

static bool worker_init = false;
worker_metrics_st *worker_metrics = NULL;

static struct context context;
static struct context *ctx = &context;

static channel_handler_t handlers;
static channel_handler_t *hdl = &handlers;

void
worker_close(struct buf_sock *s)
{
    log_info("worker core close on buf_sock %p", s);

    event_deregister(ctx->evb, s->ch->sd);

    hdl->term(s->ch);
    request_return((struct request **)&s->data);
    buf_sock_return(&s);
}

void
worker_add_conn(void)
{
    struct buf_sock *s;
    char buf[RING_ARRAY_DEFAULT_CAP]; /* buffer for discarding pipe data */
    uint32_t i;

    for (i = 0; i < RING_ARRAY_DEFAULT_CAP && ring_array_pop(&s, conn_arr) == CC_OK; ++i) {
        log_verb("Adding new buf_sock %p to worker thread", s);
        s->owner = ctx;
        s->hdl = hdl;
        event_add_read(ctx->evb, hdl->rid(s->ch), s);
    }

    pipe_recv(pipe_c, buf, i);
}

void
worker_retry_write(struct buf_sock *s, struct tcp_conn *c)
{
    event_add_write(ctx->evb, hdl->wid(c), s);
}

rstatus_t
core_worker_setup(worker_metrics_st *metrics)
{
    if (worker_init) {
        log_error("worker has already been setup, aborting");

        return CC_ERROR;
    }

    log_info("set up the %s module", WORKER_MODULE_NAME);

    ctx->timeout = 100;
    ctx->evb = event_base_create(1024, core_worker_event);
    if (ctx->evb == NULL) {
        log_crit("failed to setup worker thread core; could not create event_base");
        return CC_ERROR;
    }

    hdl->accept = tcp_accept;
    hdl->reject = tcp_reject;
    hdl->open = tcp_listen;
    hdl->term = tcp_close;
    hdl->recv = tcp_recv;
    hdl->send = tcp_send;
    hdl->rid = tcp_read_id;
    hdl->wid = tcp_write_id;

    event_add_read(ctx->evb, pipe_read_id(pipe_c), NULL);
    worker_metrics = metrics;
    WORKER_METRIC_INIT(worker_metrics);

    worker_init = true;

    return CC_OK;
}

void
core_worker_teardown(void)
{
    log_info("tear down the %s module", WORKER_MODULE_NAME);

    if (!worker_init) {
        log_warn("%s has never been setup", WORKER_MODULE_NAME);
    } else {
        event_base_destroy(&(ctx->evb));
    }
    worker_metrics = NULL;
    worker_init = false;
}

static rstatus_t
core_worker_evwait(void)
{
    int n;

    n = event_wait(ctx->evb, ctx->timeout);
    if (n < 0) {
        return n;
    }

    INCR(worker_metrics, worker_event_loop);
    INCR_N(worker_metrics, worker_event_total, n);
    time_update();

    return CC_OK;
}

void *
core_worker_evloop(void *arg)
{
    rstatus_t status;

    for(;;) {
        status = core_worker_evwait();
        if (status != CC_OK) {
            log_crit("worker core event loop exited due to failure");
            break;
        }
    }

    exit(1);
}
