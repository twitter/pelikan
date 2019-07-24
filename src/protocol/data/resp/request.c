#include "request.h"

#include "token.h"

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define REQUEST_MODULE_NAME "protocol::resp::request"

static bool request_init = false;
static request_metrics_st *request_metrics = NULL;

#define CMD_INIT(_type, _str, _narg, _nopt) {\
    .type = _type,                           \
    .bstr = { sizeof(_str) - 1, (_str) },    \
    .narg = _narg,                           \
    .nopt = _nopt },
struct command command_table[REQ_SENTINEL] = {
    { .type = REQ_UNKNOWN, .bstr = { 0, NULL }, .narg = 0, .nopt = 0 },
    REQ_BITMAP(CMD_INIT)
    REQ_HASH(CMD_INIT)
    REQ_LIST(CMD_INIT)
    REQ_ZSET(CMD_INIT)
    REQ_MISC(CMD_INIT)
};
#undef CMD_INIT

static size_t ntoken = REQ_NTOKEN;
FREEPOOL(req_pool, reqq, request);
static struct req_pool reqp;
static bool reqp_init = false;

void
request_reset(struct request *req)
{
    ASSERT(req != NULL);

    STAILQ_NEXT(req, next) = NULL;
    req->free = false;

    req->noreply = 0;
    req->serror = 0;
    req->cerror = 0;

    req->type = REQ_UNKNOWN;
    req->token->nelem = 0;
}

struct request *
request_create(void)
{
    rstatus_i status;
    struct request *req = cc_alloc(sizeof(struct request));

    if (req == NULL) {
        return NULL;
    }

    status = array_create(&req->token, ntoken, sizeof(struct element));
    if (status != CC_OK) {
        cc_free(req);
        return NULL;
    }
    request_reset(req);

    INCR(request_metrics, request_create);
    INCR(request_metrics, request_curr);

    return req;
}

static struct request *
_request_create(void)
{
    struct request *req = request_create();

    if (req != NULL) {
        INCR(request_metrics, request_free);
    }

    return req;
}

void
request_destroy(struct request **request)
{
    struct request *req = *request;
    ASSERT(req != NULL);

    INCR(request_metrics, request_destroy);
    DECR(request_metrics, request_curr);
    array_destroy(&req->token);
    cc_free(req);
    *request = NULL;
}

static void
_request_destroy(struct request **request)
{
    request_destroy(request);
    DECR(request_metrics, request_free);
}

static void
request_pool_destroy(void)
{
    struct request *req, *treq;

    if (!reqp_init) {
        log_warn("request pool was never created, ignore");
    }

    log_info("destroying request pool: free %"PRIu32, reqp.nfree);

    FREEPOOL_DESTROY(req, treq, &reqp, next, _request_destroy);
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

    FREEPOOL_PREALLOC(req, &reqp, max, next, _request_create);
    if (reqp.nfree < max) {
        log_crit("cannot preallocate request pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

struct request *
request_borrow(void)
{
    struct request *req;

    FREEPOOL_BORROW(req, &reqp, next, _request_create);
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
        int i;
        ntoken = option_uint(&options->request_ntoken);
        for (i = 1; i < REQ_SENTINEL; i++) { /* update nopt based on ntoken */
            if (command_table[i].nopt == -1) {
                command_table[i].nopt = ntoken - command_table[i].narg;
            }
        }
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

    ntoken = REQ_NTOKEN;
    request_pool_destroy();
    request_metrics = NULL;

    request_init = false;
}
