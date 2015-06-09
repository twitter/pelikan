#include <protocol/memcache/bb_request.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_pool.h>

#define REQUEST_MODULE_NAME "protocol::memcache::request"

static bool request_init = false;
static request_metrics_st *request_metrics = NULL;

FREEPOOL(req_pool, reqq, request);
static struct req_pool reqp;
static bool reqp_init = false;

void
request_setup(request_metrics_st *metrics)
{
    log_info("set up the %s module", REQUEST_MODULE_NAME);

    request_metrics = metrics;
    REQUEST_METRIC_INIT(request_metrics);

    if (request_init) {
        log_warn("%s has already been setup, overwrite", REQUEST_MODULE_NAME);
    }
    request_init = true;
}

void
request_teardown(void)
{
    log_info("tear down the %s module", REQUEST_MODULE_NAME);

    if (!request_init) {
        log_warn("%s has never been setup", REQUEST_MODULE_NAME);
    }
    request_metrics = NULL;
    request_init = false;
}

void
request_reset(struct request *req)
{
    ASSERT(req != NULL && req->keys != NULL);

    STAILQ_NEXT(req, next) = NULL;
    req->free = false;

    req->rstate = PARSING;
    req->pstate = REQ_HDR;
    req->tstate = 0;
    req->verb = REQ_UNKNOWN;

    req->keys->nelem = 0;
    bstring_init(&req->vstr);
    req->flag = 0;
    req->expiry = 0;
    req->vlen = 0;
    req->delta = 0;
    req->cas = 0;

    req->noreply = 0;
    req->serror = 0;
    req->cerror = 0;
    req->swallow = 0;
}

struct request *
request_create(void)
{
    rstatus_t status;
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


void
request_pool_create(uint32_t max)
{
    uint32_t i;
    struct request *req;

    if (reqp_init) {
        log_warn("request pool has already been created, ignore");

        return;
    }

    log_info("creating request pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&reqp, max);
    reqp_init = true;

    /* preallocating, see notes in cc_fbuf.c */
    if (max == 0) {
        return;
    }

    for (i = 0; i < max; ++i) {
        req = request_create();
        if (req == NULL) {
            log_crit("cannot preallocate request pool due to OOM, abort");
            exit(EXIT_FAILURE);
        }
        req->free = true;
        FREEPOOL_RETURN(&reqp, req, next);
        INCR(request_metrics, request_free);
    }
}

void
request_pool_destroy(void)
{
    struct request *req, *treq;

    if (reqp_init) {
        log_info("destroying request pool: free %"PRIu32, reqp.nfree);

        FREEPOOL_DESTROY(req, treq, &reqp, next, request_destroy);
        reqp_init = false;
    } else {
        log_warn("request pool was never created, ignore");
    }
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
    FREEPOOL_RETURN(&reqp, req, next);

    *request = NULL;
}
