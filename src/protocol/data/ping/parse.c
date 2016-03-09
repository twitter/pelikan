#include <protocol/data/ping/parse.h>

#include <cc_debug.h>

#define PARSE_MODULE_NAME "protocol::ping::parse"

static bool parse_init = false;
static parse_req_metrics_st *parse_req_metrics = NULL;
static parse_rsp_metrics_st *parse_rsp_metrics = NULL;

void
parse_setup(parse_req_metrics_st *req, parse_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", PARSE_MODULE_NAME);

    parse_req_metrics = req;
    if (parse_req_metrics != NULL) {
        PARSE_REQ_METRIC_INIT(parse_req_metrics);
    }
    parse_rsp_metrics = rsp;
    if (parse_rsp_metrics != NULL) {
        PARSE_RSP_METRIC_INIT(parse_rsp_metrics);
    }

    if (parse_init) {
        log_warn("%s has already been setup, overwrite", PARSE_MODULE_NAME);
    }
    parse_init = true;
}

void
parse_teardown(void)
{
    log_info("tear down the %s module", PARSE_MODULE_NAME);

    if (!parse_init) {
        log_warn("%s has never been setup", PARSE_MODULE_NAME);
    }
    parse_req_metrics = NULL;
    parse_rsp_metrics = NULL;
    parse_init = false;
}
