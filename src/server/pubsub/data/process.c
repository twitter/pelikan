#include "process.h"

#include "protocol/data/redis_include.h"
#include "storage/pubsub/listener.h"
#include "storage/pubsub/topic.h"

#include "core/data/pubsub.h"

#include <buffer/cc_dbuf.h>
#include <cc_array.h>
#include <cc_event.h>
#include <stream/cc_sockio.h>

#define PUBSUB_PROCESS_MODULE_NAME "pubsub::process"

#define MESSAGE "message"
#define SUBSCRIBE "subscribe"
#define UNSUBSCRIBE "unsubscribe"

static struct element el_message =
    {.type = ELEM_BULK, .bstr = {sizeof(MESSAGE) - 1, MESSAGE}};
static struct element el_subscribe =
    {.type = ELEM_BULK, .bstr = {sizeof(SUBSCRIBE) - 1, SUBSCRIBE}};
static struct element el_unsubscribe =
    {.type = ELEM_BULK, .bstr = {sizeof(UNSUBSCRIBE) - 1, UNSUBSCRIBE}};


typedef void (* command_fn)(struct response *, struct request *, struct buf_sock *);
command_fn command_registry[REQ_SENTINEL];

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;


/* "publish topic msg"
 * reply: # listeners subscribed
 */
static void
command_publish(struct response *rsp, struct request *req, struct buf_sock *s)
{
    struct element *el_t, *el_m; /* topic & message */
    struct element el_r = {.type = ELEM_INT}; /* reply, an integer */
    struct listener *l;
    struct topic *t;
    uint32_t nsub = 0;

    el_t = array_get(req->token, 1);
    if (el_t->type != ELEM_BULK) {
        /* handle error */
    };
    el_m = array_get(req->token, 2);
    if (el_m->type != ELEM_BULK) {
        /* handle error */
    };

    log_verb("publish from buf_sock %p", s);

    l = listener_get(s);
    if (l != NULL) {
        log_error("found listener at %p: subscriber cannot publish", l);
        /* handle error */
    }

    t = topic_get(&el_t->bstr);
    if (t == NULL) {
        log_verb("no listener on topic %.*s, ignore", el_t->bstr.len,
                el_t->bstr.data);
    } else { /* copy message to listener's buffer, this should be optimized later */
        struct index_node *node;

        nsub = t->nsub;
        TAILQ_FOREACH(node, t->idx, i_tqe) {
            l = (struct listener *)node->obj;
            struct buf_sock *ss = l->s;
            compose_array_header(&ss->wbuf, 3);
            compose_element(&ss->wbuf, &el_message);
            compose_element(&ss->wbuf, el_t);
            compose_element(&ss->wbuf, el_m);
            /* register listener's fd for write event */
            event_add_write(ctx->evb, ss->hdl->wid(ss->ch), ss);
        }
    }

    el_r.num = nsub;
    compose_element(&s->wbuf, &el_r);
}

/* "subscribe topic [topic ...]"
 * no reply
 */
static void
command_subscribe(struct response *rsp, struct request *req, struct buf_sock *s)
{
    struct element *el;
    struct element el_s = {.type = ELEM_INT}; /* # topics subscribed, an integer */
    struct listener *l;
    struct topic *t;
    uint32_t ntopic = req->token->nelem - 1;

    log_verb("subscribe buf_sock %p from topics", s);

    l = listener_get(s);
    if (l == NULL) {
        log_verb("create new listener for %p", s);
        l = listener_create(s);
        listener_put(l);
    }

    for (int i = 1; i <= ntopic; i++) {
        el = array_get(req->token, i);
        if (el->type != ELEM_BULK) {
            /* handle error */
        };

        t = topic_get(&el->bstr);
        if (t == NULL) {
            log_verb("creating topic %.*s", el->bstr.len, el->bstr.data);
            t = topic_create(&el->bstr);
            /* handle error */
            topic_put(t);
        }

        log_verb("subscribing to topic %.*s", el->bstr.len, el->bstr.data);
        if (!topic_add_listener(t, l)) {
            log_debug("listener not added");
        }
        if (!listener_add_topic(l, t)) {
            log_debug("topic not added");
        }

        el_s.num = l->ntopic;
        compose_array_header(&s->wbuf, 3);
        compose_element(&s->wbuf, &el_subscribe);
        compose_element(&s->wbuf, el);
        compose_element(&s->wbuf, &el_s);
    }
}

/* "unsubscribe topic [topic ...]"
 * no reply
 * (not supporting unsubscribe from all right now)
 */
static void
command_unsubscribe(struct response *rsp, struct request *req, struct buf_sock *s)
{
    struct element *el;
    struct element el_s = {.type = ELEM_INT}; /* # topics subscribed, an integer */
    struct listener *l;
    struct topic *t;
    uint32_t ntopic = req->token->nelem - 1;

    log_verb("unsubscribe buf_sock %p from topics", s);

    l = listener_get(s);
    if (l == NULL) {
        log_info("listener not found for %p", s);
        return;
    }

    for (int i = 1; i <= ntopic; i++) {
        el = array_get(req->token, i);
        if (el->type != ELEM_BULK) {
            /* handle error */
        };

        t = topic_get(&el->bstr);
        if (t == NULL) {
            log_debug("topic %.*s does not exist", el->bstr.len, el->bstr.data);
            continue;
        }

        log_verb("unsubscribing from topic %.*s", el->bstr.len, el->bstr.data);
        listener_del_topic(l, t);
        topic_del_listener(t, l);

        if (t->nsub == 0) { /* remove topic that nobody listens to */
            log_verb("deleting topic %.*s", el->bstr.len, el->bstr.data);
            topic_delete(&t->name);
            topic_destroy(&t);
        }

        el_s.num = l->ntopic;
        compose_array_header(&s->wbuf, 3);
        compose_element(&s->wbuf, &el_unsubscribe);
        compose_element(&s->wbuf, el);
        compose_element(&s->wbuf, &el_s);
    }
}

void
process_setup(process_metrics_st *metrics)
{
    log_info("set up the %s module", PUBSUB_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 PUBSUB_PROCESS_MODULE_NAME);
    }

    listener_setup(16);
    topic_setup(16);

    command_registry[REQ_PUBLISH] = command_publish;
    command_registry[REQ_SUBSCRIBE] = command_subscribe;
    command_registry[REQ_UNSUBSCRIBE] = command_unsubscribe;

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

    listener_teardown();
    topic_teardown();

    command_registry[REQ_PUBLISH] = NULL;
    command_registry[REQ_SUBSCRIBE] = NULL;
    command_registry[REQ_UNSUBSCRIBE] = NULL;

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

    log_verb("processing request type %d", req->type);
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
    struct listener *l;

    log_verb("post-error processing");

    l = listener_get(s);
    if (l != NULL) {
        listener_delete(s);
        /* TODO: unsubscribe automatically from all */
        listener_destroy(&l);
    }

    /* normalize buffer size */
    buf_reset(s->rbuf);
    dbuf_shrink(&s->rbuf);
    buf_reset(s->wbuf);
    dbuf_shrink(&s->wbuf);

    return 0;
}
