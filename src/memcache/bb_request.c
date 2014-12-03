#include <memcache/bb_request.h>

#include <bb_stats.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_pool.h>

FREEPOOL(req_pool, reqq, request);
static struct req_pool reqp;

static bool reqp_init = false;

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

    INCR(request_create);

    return req;
}

void
request_destroy(struct request **request)
{
    struct request *req = *request;
    ASSERT(req != NULL);

    INCR(request_destroy);
    array_destroy(&req->keys);
    cc_free(req);
    *request = NULL;
}


void
request_pool_create(uint32_t max)
{
    /* TODO(yao): add a pre-alloc interface */
    if (!reqp_init) {
        log_info("creating request pool: max %"PRIu32, max);

        FREEPOOL_CREATE(&reqp, max);
        reqp_init = true;
    } else {
        log_warn("request pool has already been created, ignore");
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

    INCR(request_borrow);
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

    INCR(request_free);
    INCR(request_return);
    log_vverb("return req %p", req);

    req->free = true;
    FREEPOOL_RETURN(&reqp, req, next);

    *request = NULL;
}
