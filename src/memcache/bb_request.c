#include <memcache/bb_request.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_pool.h>

FREEPOOL(req_pool, reqq, request);
struct req_pool reqp;

void
request_reset(struct request *req)
{
    ASSERT(req != NULL && req->keys != NULL);

    req->rstate = PARSING;
    req->pstate = REQ_HDR;
    req->tstate = 0;
    req->verb = UNKNOWN;

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

    return req;
}

void
request_destroy(struct request *req)
{
    ASSERT(req != NULL);

    array_destroy(&req->keys);
    cc_free(req);
}


void
request_pool_create(uint32_t max)
{
    log_debug(LOG_INFO, "creating request pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&reqp, max);
}

void
request_pool_destroy(void)
{
    struct request *req, *treq;

    log_debug(LOG_INFO, "destroying request pool: free %"PRIu32, reqp.nfree);

    FREEPOOL_DESTROY(req, treq, &reqp, next, request_destroy);
}

struct request *
request_borrow(void)
{
    struct request *req;

    FREEPOOL_BORROW(req, &reqp, next, request_create);
    if (req == NULL) {
        log_debug(LOG_DEBUG, "borrow req failed: OOM %d");

        return NULL;
    }

    log_debug(LOG_VVERB, "borrowing req %p", req);

    return req;
}

void
request_return(struct request *req)
{
    log_debug(LOG_VVERB, "return req %p: free %"PRIu32, req, reqp.nfree);

    FREEPOOL_RETURN(&reqp, req, next);
}
