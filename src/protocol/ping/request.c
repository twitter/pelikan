#include <protocol/ping/request.h>

#include <cc_debug.h>

#define REQUEST_MODULE_NAME "protocol::ping::request"

static bool request_init = false;
static request_metrics_st *request_metrics = NULL;

void
request_setup(request_metrics_st *metrics)
{
    log_info("set up the %s module", REQUEST_MODULE_NAME);

    request_metrics = metrics;
    if (metrics != NULL) {
        REQUEST_METRIC_INIT(request_metrics);
    }

    if (request_init) {
        log_warn("%s has already been setup, overwrite", REQUEST_MODULE_NAME);
    }
    request_init = true;
}

void
request_teardown(void)
{
    log_info("tear down the %s module", REQUEST_MODULE_NAME);

    if (!request_init) {
        log_warn("%s has never been setup", REQUEST_MODULE_NAME);
    }
    request_metrics = NULL;
    request_init = false;
}
