#include <protocol/memcache/response.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define RESPONSE_MODULE_NAME "protocol::memcache::response"

static bool response_init = false;
static response_metrics_st *response_metrics = NULL;

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring rsp_strings[] = {
    RSP_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

FREEPOOL(rsp_pool, rspq, response);
static struct rsp_pool rspp;
static bool rspp_init = false;

void
response_setup(response_metrics_st *metrics)
{
    log_info("set up the %s module", RESPONSE_MODULE_NAME);

    response_metrics = metrics;
    if (metrics != NULL) {
        RESPONSE_METRIC_INIT(response_metrics);
    }

    if (response_init) {
        log_warn("%s has already been setup, overwrite", RESPONSE_MODULE_NAME);
    }
    response_init = true;
}

void
response_teardown(void)
{
    log_info("tear down the %s module", RESPONSE_MODULE_NAME);

    if (!response_init) {
        log_warn("%s has never been setup", RESPONSE_MODULE_NAME);
    }
    response_metrics = NULL;
    response_init = false;
}

void
response_reset(struct response *rsp)
{
    ASSERT(rsp != NULL);

    STAILQ_NEXT(rsp, next) = NULL;
    rsp->free = false;

    rsp->rstate = RSP_PARSING;
    rsp->pstate = RSP_HDR;
    rsp->type = RSP_UNKNOWN;

    bstring_init(&rsp->key);
    bstring_init(&rsp->vstr);
    rsp->vint = 0;
    rsp->vcas = 0;
    rsp->met = NULL;
    rsp->flag = 0;

    rsp->cas = 0;
    rsp->num = 0;
    rsp->val = 0;
    rsp->error = 0;
}

struct response *
response_create(void)
{
    struct response *rsp = cc_alloc(sizeof(struct response));

    if (rsp == NULL) {
        return NULL;
    }

    response_reset(rsp);

    INCR(response_metrics, response_create);

    return rsp;
}

void
response_destroy(struct response **response)
{
    struct response *rsp = *response;
    ASSERT(rsp != NULL);

    INCR(response_metrics, response_destroy);
    cc_free(rsp);
    *response = NULL;
}


void
response_pool_create(uint32_t max)
{
    uint32_t i;
    struct response *rsp;

    if (rspp_init) {
        log_warn("response pool has already been created, ignore");

        return;
    }

    log_info("creating response pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&rspp, max);
    rspp_init = true;

    /* preallocating, see notes in cc_fbuf.c */
    if (max == 0) {
        return;
    }

    for (i = 0; i < max; ++i) {
        rsp = response_create();
        if (rsp == NULL) {
            log_crit("cannot preallocate response pool due to OOM, abort");
            exit(EXIT_FAILURE);
        }
        rsp->free = true;
        FREEPOOL_RETURN(&rspp, rsp, next);
        INCR(response_metrics, response_free);
    }
}

void
response_pool_destroy(void)
{
    struct response *rsp, *trsp;

    if (rspp_init) {
        log_info("destroying response pool: free %"PRIu32, rspp.nfree);

        FREEPOOL_DESTROY(rsp, trsp, &rspp, next, response_destroy);
        rspp_init = false;
    } else {
        log_warn("response pool was never created, ignore");
    }
}

struct response *
response_borrow(void)
{
    struct response *rsp;

    FREEPOOL_BORROW(rsp, &rspp, next, response_create);
    if (rsp == NULL) {
        log_debug("borrow rsp failed: OOM %d");

        return NULL;
    }
    response_reset(rsp);

    DECR(response_metrics, response_free);
    INCR(response_metrics, response_borrow);
    log_vverb("borrowing rsp %p", rsp);

    return rsp;
}

void
response_return(struct response **response)
{
    struct response *rsp = *response;

    if (rsp == NULL) {
        return;
    }

    INCR(response_metrics, response_free);
    INCR(response_metrics, response_return);
    log_vverb("return rsp %p", rsp);

    rsp->free = true;
    FREEPOOL_RETURN(&rspp, rsp, next);

    *response = NULL;
}
