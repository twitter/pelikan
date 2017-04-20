#include "compose.h"

#include <buffer/cc_dbuf.h>
#include <cc_debug.h>

#define COMPOSE_MODULE_NAME "protocol::ping::compose"

static bool compose_init = false;
static compose_req_metrics_st *compose_req_metrics = NULL;
static compose_rsp_metrics_st *compose_rsp_metrics = NULL;

void
compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", COMPOSE_MODULE_NAME);

    if (compose_init) {
        log_warn("%s has already been setup, overwrite", COMPOSE_MODULE_NAME);
    }

    compose_req_metrics = req;
    compose_rsp_metrics = rsp;

    compose_init = true;
}

void
compose_teardown(void)
{
    log_info("tear down the %s module", COMPOSE_MODULE_NAME);

    if (!compose_init) {
        log_warn("%s has never been setup", COMPOSE_MODULE_NAME);
    }
    compose_req_metrics = NULL;
    compose_rsp_metrics = NULL;
    compose_init = false;
}

compose_rstatus_t
compose_req(struct buf **buf)
{
    log_verb("composing request to buf %p", buf);

    if (buf_wsize(*buf) < sizeof(REQUEST_UPPER) && dbuf_double(buf) != CC_OK) {
        log_debug("failed to double buf %p");
        INCR(compose_req_metrics, request_compose_ex);

        return COMPOSE_ENOMEM;
    }

    buf_write(*buf, REQUEST_UPPER, REQ_LEN);
    INCR(compose_req_metrics, request_compose);
    return COMPOSE_OK;
}

compose_rstatus_t
compose_rsp(struct buf **buf)
{
    log_verb("composing response to buf %p", buf);

    if (buf_wsize(*buf) < sizeof(RESPONSE) && dbuf_double(buf) != CC_OK) {
        log_debug("failed to double buf %p");
        INCR(compose_rsp_metrics, response_compose_ex);

        return COMPOSE_ENOMEM;
    }

    buf_write(*buf, RESPONSE, RSP_LEN);
    INCR(compose_rsp_metrics, response_compose);
    return COMPOSE_OK;
}
