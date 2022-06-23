#include "response.h"

#include "token.h"

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define RESPONSE_MODULE_NAME "protocol::resp-tw::response"

static bool response_init = false;
static response_metrics_st *response_metrics = NULL;

static size_t ntoken = RSP_NTOKEN;
FREEPOOL(rsp_pool, rspq, response);
static struct rsp_pool rspp;
static bool rspp_init = false;

void
response_reset(struct response *rsp)
{
    ASSERT(rsp != NULL);

    STAILQ_NEXT(rsp, next) = NULL;
    rsp->free = false;

    rsp->serror = false;

    rsp->type = ELEM_UNKNOWN;
    rsp->token->nelem = 0;
    rsp->attrs->nelem = 0;
}

struct response *
response_create(void)
{
    rstatus_i status;
    struct response *rsp = cc_alloc(sizeof(struct response));

    if (rsp == NULL) {
        return NULL;
    }

    status = array_create(&rsp->token, ntoken, sizeof(struct element));
    if (status != CC_OK) {
        cc_free(rsp);
        return NULL;
    }

    status = array_create(&rsp->attrs, ntoken/2, sizeof(struct attribute_entry));
    if (status != CC_OK) {
        cc_free(rsp);
        array_destroy(&rsp->token);
        return NULL;
    }

    response_reset(rsp);

    INCR(response_metrics, response_create);
    INCR(response_metrics, response_curr);

    return rsp;
}

static struct response *
_response_create(void)
{
    struct response *rsp = response_create();

    if (rsp != NULL) {
        INCR(response_metrics, response_free);
    }

    return rsp;
}

void
response_destroy(struct response **response)
{
    struct response *rsp = *response;
    ASSERT(rsp != NULL);

    INCR(response_metrics, response_destroy);
    DECR(response_metrics, response_curr);
    array_destroy(&rsp->token);
    array_destroy(&rsp->attrs);
    cc_free(rsp);
    *response = NULL;
}

static void
_response_destroy(struct response **response)
{
    response_destroy(response);
    DECR(response_metrics, response_free);
}

static void
response_pool_destroy(void)
{
    struct response *rsp, *trsp;

    if (rspp_init) {
        log_info("destroying response pool: free %"PRIu32, rspp.nfree);

        FREEPOOL_DESTROY(rsp, trsp, &rspp, next, _response_destroy);
        rspp_init = false;
    } else {
        log_warn("response pool was never created, ignore");
    }
}


static void
response_pool_create(uint32_t max)
{
    struct response *rsp;

    if (rspp_init) {
        log_warn("response pool has already been created, re-creating");

        response_pool_destroy();
    }

    log_info("creating response pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&rspp, max);
    rspp_init = true;

    FREEPOOL_PREALLOC(rsp, &rspp, max, next, _response_create);
    if (rspp.nfree < max) {
        log_crit("cannot preallocate response pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

struct response *
response_borrow(void)
{
    struct response *rsp;

    FREEPOOL_BORROW(rsp, &rspp, next, _response_create);
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

/*
 * Return a single response object
 */
void
response_return(struct response **response)
{
    ASSERT(response != NULL);

    struct response *rsp = *response;

    if (rsp == NULL) {
        return;
    }

    INCR(response_metrics, response_free);
    INCR(response_metrics, response_return);
    log_vverb("return rsp %p", rsp);

    rsp->free = true;
    FREEPOOL_RETURN(rsp, &rspp, next);

    *response = NULL;
}

void
response_setup(response_options_st *options, response_metrics_st *metrics)
{
    uint32_t max = RSP_POOLSIZE;

    log_info("set up the %s module", RESPONSE_MODULE_NAME);

    if (response_init) {
        log_warn("%s has already been setup, overwrite", RESPONSE_MODULE_NAME);
    }

    response_metrics = metrics;

    if (options != NULL) {
        ntoken = option_uint(&options->response_ntoken);
        max = option_uint(&options->response_poolsize);
    }

    response_pool_create(max);

    response_init = true;
}

void
response_teardown(void)
{
    log_info("tear down the %s module", RESPONSE_MODULE_NAME);

    if (!response_init) {
        log_warn("%s has never been setup", RESPONSE_MODULE_NAME);
    }

    ntoken = RSP_NTOKEN;
    response_pool_destroy();
    response_metrics = NULL;

    response_init = false;
}

