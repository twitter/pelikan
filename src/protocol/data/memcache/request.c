#include "request.h"

#include <cc_debug.h>
#include <cc_pool.h>

#define REQUEST_MODULE_NAME "protocol::memcache::request"

static bool request_init = false;
static request_metrics_st *request_metrics = NULL;

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring req_strings[] = {
    REQ_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

FREEPOOL(req_pool, reqq, request);
static struct req_pool reqp;
static bool reqp_init = false;

void
request_reset(struct request *req)
{
    ASSERT(req != NULL && req->keys != NULL);

    STAILQ_NEXT(req, next) = NULL;
    req->free = false;

    req->rstate = REQ_PARSING;
    req->type = REQ_UNKNOWN;

    req->keys->nelem = 0;
    bstring_init(&(req->vstr));
    req->nfound = 0;

    req->flag = 0;
    req->expiry = 0;
    req->vlen = 0;
    req->delta = 0;
    req->vcas = 0;

    req->nremain = 0;
    req->reserved = NULL;

    req->partial = 0;
    req->first = 0;
    req->swallow = 0;
    req->noreply = 0;
    req->val = 0;
    req->serror = 0;
    req->cerror = 0;
}

struct request *
request_create(void)
{
    rstatus_i status;
    struct request *req = cc_alloc(sizeof(struct request));

    if (req == NULL) {
        return NULL;
    }

    status = array_create(&req->keys, MAX_BATCH_SIZE, sizeof(struct bstring));
    if (status != CC_OK) {
        return NULL;
    }
    request_reset(req);

    INCR(request_metrics, request_create);

    return req;
}

void
request_destroy(struct request **request)
{
    struct request *req = *request;
    ASSERT(req != NULL);

    INCR(request_metrics, request_destroy);
    array_destroy(&req->keys);
    cc_free(req);
    *request = NULL;
}

static void
request_pool_destroy(void)
{
    struct request *req, *treq;

    if (!reqp_init) {
        log_warn("request pool was never created, ignore");
    }

    log_info("destroying request pool: free %"PRIu32, reqp.nfree);

    FREEPOOL_DESTROY(req, treq, &reqp, next, request_destroy);
    reqp_init = false;
}

static void
request_pool_create(uint32_t max)
{
    struct request *req;

    if (reqp_init) {
        log_warn("request pool has already been created, re-creating");

        request_pool_destroy();
    }

    log_info("creating request pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&reqp, max);
    reqp_init = true;

    FREEPOOL_PREALLOC(req, &reqp, max, next, request_create);
    if (reqp.nfree < max) {
        log_crit("cannot preallocate request pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
    UPDATE_VAL(request_metrics, request_free, max);
}

struct request *
request_borrow(void)
{
    struct request *req;

    FREEPOOL_BORROW(req, &reqp, next, request_create);
    if (req == NULL) {
        log_debug("borrow req failed: OOM %d");

        return NULL;
    }
    request_reset(req);

    DECR(request_metrics, request_free);
    INCR(request_metrics, request_borrow);
    log_vverb("borrowing req %p", req);

    return req;
}

void
request_return(struct request **request)
{
    struct request *req = *request;

    if (req == NULL) {
        return;
    }

    INCR(request_metrics, request_free);
    INCR(request_metrics, request_return);
    log_vverb("return req %p", req);

    req->free = true;
    FREEPOOL_RETURN(req, &reqp, next);

    *request = NULL;
}

void
request_setup(request_options_st *options, request_metrics_st *metrics)
{
    uint32_t max = REQ_POOLSIZE;

    log_info("set up the %s module", REQUEST_MODULE_NAME);

    if (request_init) {
        log_warn("%s has already been setup, overwrite", REQUEST_MODULE_NAME);
    }

    request_metrics = metrics;

    if (options != NULL) {
        max = option_uint(&options->request_poolsize);
    }
    request_pool_create(max);

    request_init = true;
}

void
request_teardown(void)
{
    log_info("tear down the %s module", REQUEST_MODULE_NAME);

    if (!request_init) {
        log_warn("%s has never been setup", REQUEST_MODULE_NAME);
    }
    request_pool_destroy();
    request_metrics = NULL;

    request_init = false;
}
