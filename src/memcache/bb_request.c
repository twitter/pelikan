#include <memcache/bb_request.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_log.h>

STAILQ_HEAD(rq, request);

static struct reqpool {
    struct rq free_rq;
    struct rq used_rq;
    uint32_t  nfree;
    uint32_t  nused;
    uint32_t  nmax;
} reqpool;

void
request_reset(struct request *req)
{
    req->rstate = PARSING;
    req->pstate = VERB;
    req->tstate = 0;
    req->verb = UNKNOWN;

    req->keys->nelem = 0;
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


rstatus_t
request_pool_create(uint32_t low_wm, uint32_t high_wm)
{
    uint32_t n;
    struct request *req;

    ASSERT(high_wm >= low_wm);

    STAILQ_INIT(&reqpool.free_rq);
    STAILQ_INIT(&reqpool.used_rq);
    reqpool.nmax = high_wm;
    reqpool.nfree = 0;
    reqpool.nused = 0;

    for (n = 0; n < low_wm; ++n) {
        req = cc_alloc(sizeof(struct request));
        if (req == NULL) {
            log_error("request pool create failed: OOM, allocated %"PRIu32", "
                    "target %"PRIu32, n, low_wm);

            request_pool_destroy();
            return CC_ENOMEM;
        }
        STAILQ_INSERT_TAIL(&reqpool.free_rq, req, next);
        reqpool.nfree++;
    }

    log_debug(LOG_INFO, "creating request pool: allocated %"PRIu32", max %"
            PRIu32, reqpool.nfree, reqpool.nmax);

    return CC_OK;
}

void
request_pool_destroy(void)
{
    uint32_t n;
    struct request *req;

    log_debug(LOG_INFO, "destroying request pool: free %"PRIu32", used %"PRIu32,
            reqpool.nfree, reqpool.nused);

    for (n = 0; n < reqpool.nfree; ++n) {
        req = STAILQ_FIRST(&reqpool.free_rq);
        reqpool.nfree--;
        STAILQ_REMOVE_HEAD(&reqpool.free_rq, next);
        request_destroy(req);
    }

    for (n = 0; n < reqpool.nused; ++n) {
        req = STAILQ_FIRST(&reqpool.used_rq);
        reqpool.nused--;
        STAILQ_REMOVE_HEAD(&reqpool.used_rq, next);
        request_destroy(req);
    }
}

struct request *
request_get(void)
{
    struct request *req;

    if (reqpool.nfree > 0) {
        req = STAILQ_FIRST(&reqpool.free_rq);
        reqpool.nfree--;
        STAILQ_REMOVE_HEAD(&reqpool.free_rq, next);
        STAILQ_INSERT_TAIL(&reqpool.used_rq, req, next);
        reqpool.nused++;

        log_debug(LOG_VVERB, "getting req %p from reqpool.free_rq", req);
    } else if (reqpool.nfree + reqpool.nused < reqpool.nmax) {
        req = request_create();
        STAILQ_INSERT_TAIL(&reqpool.used_rq, req, next);
        reqpool.nused++;

        log_debug(LOG_VVERB, "creating new req %p", req);
    } else {
        req = NULL;

        log_debug(LOG_VVERB, "returning NULL req: nreq exceeding limit %d",
                reqpool.nmax);
    }

    return req;
}

void
request_put(struct request *req)
{
    STAILQ_INSERT_TAIL(&reqpool.free_rq, req, next);
    reqpool.nfree++;

    log_debug(LOG_VVERB, "put req %p to reqpool.free_rq: free %"PRIu32, req,
            reqpool.nfree);
}
