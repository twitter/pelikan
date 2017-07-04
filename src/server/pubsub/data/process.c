#include "process.h"

#include "protocol/data/redis_include.h"
#include "storage/pubsub/listener.h"
#include "storage/pubsub/topic.h"

#include <buffer/cc_dbuf.h>
#include <cc_array.h>
#include <stream/cc_sockio.h>

#define PUBSUB_PROCESS_MODULE_NAME "pubsub::process"

command_fn command_registry[REQ_SENTINEL];

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;

static struct listener_ht *lht;
static struct topic_ht *tht;


/* "subscribe topic [topic ...]" */
static void
command_subscribe(struct response *rsp, struct request *req, struct buf_sock *s)
{
    struct element *el;
    struct listener *l;
    struct topic *t;
    uint32_t ntopic = req->token->nelem - 1;

    l = listener_ht_get(s->ch, lht);
    if (l == NULL) {
        l = listener_create(s->ch, s->hdl);
        listener_ht_put(l, lht);
    }

    for (int i = 1; i < ntopic; i++) {
        el = array_get(req->token, i);
        if (el->type != ELEM_BULK) {
            /* handle client error */
        };

        t = topic_ht_get(&el->bstr, tht);
        if (t == NULL) {
            t = topic_create(&el->bstr);
        }
        listener_add_topic(l, t);
    }

    rsp->type = ELEM_STR;
    el = array_push(rsp->token);
    el->type = ELEM_STR;
    el->bstr = str2bstr(RSP_STR_OK);
}

void
process_setup(process_metrics_st *metrics)
{
    log_info("set up the %s module", PUBSUB_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 PUBSUB_PROCESS_MODULE_NAME);
    }

    lht = listener_ht_create(16);
    tht = topic_ht_create(16);

    command_registry[REQ_SUBSCRIBE] = command_subscribe;

    process_metrics = metrics;
    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", PUBSUB_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", PUBSUB_PROCESS_MODULE_NAME);
    }

    listener_ht_destroy(&lht);
    topic_ht_destroy(&tht);

    command_registry[REQ_SUBSCRIBE] = NULL;

    process_metrics = NULL;
    process_init = false;
}

static void
process_request_sock(struct response *rsp, struct request *req, struct buf_sock *s)
{
    log_verb("processing req %p, write rsp to %p", req, rsp);
    INCR(process_metrics, process_req);

    if (command_registry[req->type] == NULL) {
        /* return error */
    }

    command_registry[req->type](rsp, req, s);
}

int
pubsub_process_read(struct buf_sock *s)
{
    int status;
    struct request *req;
    struct response *rsp;

    log_verb("post-read processing");

    req = request_borrow();
    rsp = response_borrow();

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(s->rbuf) > 0) {
        /* stage 1: parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(s->rbuf));

        status = parse_req(req, s->rbuf);
        if (status == PARSE_EUNFIN) {
            buf_lshift(s->rbuf);
            return 0;
        }
        if (status != PARSE_OK) {
            /* parsing errors are all client errors, since we don't know
             * how to recover from client errors in this condition (we do not
             * have a valid request so we don't know where the invalid request
             * ends), we should close the connection
             */
            log_warn("illegal request received, status: %d", status);
            return -1;
        }

        /* stage 2: processing- check for quit, allocate response(s), process */

        /* quit is special, no processing/resposne expected */
        if (req->type == REQ_QUIT) {
            log_info("peer called quit");
            return -1;
        }

        /* actual processing */
        process_request_sock(rsp, req, s);

        /* stage 3: write response(s) if necessary */
        compose_rsp(&s->wbuf, rsp);

        /* noreply means no need to write to buffers */

        /* logging, clean-up */
    }

    request_return(&req);
    response_return(&rsp);

    return 0;
}


int
pubsub_process_write(struct buf_sock *s)
{
    log_verb("post-write processing");

    buf_lshift(s->rbuf);
    buf_lshift(s->wbuf);
    dbuf_shrink(&s->rbuf);
    dbuf_shrink(&s->wbuf);

    return 0;
}


int
pubsub_process_error(struct buf_sock *s)
{
    struct request *req;
    struct response *rsp;

    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(s->rbuf);
    dbuf_shrink(&s->rbuf);
    buf_reset(s->wbuf);
    dbuf_shrink(&s->wbuf);

    return 0;
}
