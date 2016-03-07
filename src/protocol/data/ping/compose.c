#include <protocol/data/ping/compose.h>

#include <cc_debug.h>

#define COMPOSE_MODULE_NAME "protocol::ping::compose"

static bool compose_init = false;
static compose_req_metrics_st *compose_req_metrics = NULL;
static compose_rsp_metrics_st *compose_rsp_metrics = NULL;

void
compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", COMPOSE_MODULE_NAME);

    compose_req_metrics = req;
    if (compose_req_metrics != NULL) {
        COMPOSE_REQ_METRIC_INIT(compose_req_metrics);
    }
    compose_rsp_metrics = rsp;
    if (compose_rsp_metrics != NULL) {
        COMPOSE_RSP_METRIC_INIT(compose_rsp_metrics);
    }

    if (compose_init) {
        log_warn("%s has already been setup, overwrite", COMPOSE_MODULE_NAME);
    }
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
