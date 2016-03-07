#include <protocol/data/ping/response.h>

#include <cc_debug.h>

#define RESPONSE_MODULE_NAME "protocol::ping::response"

static bool response_init = false;
static response_metrics_st *response_metrics = NULL;

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
