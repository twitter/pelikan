#include <pingserver/process.h>

#include <pingserver/stats.h>
#include <util/procinfo.h>

#include <cc_debug.h>
#include <cc_print.h>

#define PINGSERVER_PROCESS_MODULE_NAME "pingserver::process"

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;

void
process_setup(process_metrics_st *metrics)
{
    log_info("set up the %s module", PINGSERVER_PROCESS_MODULE_NAME);
    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 PINGSERVER_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;
    PROCESS_METRIC_INIT(process_metrics);
    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", PINGSERVER_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", PINGSERVER_PROCESS_MODULE_NAME);
    }

    process_metrics = NULL;
    process_init = false;
}

static void
_process_ping(struct response *rsp, struct request *req)
{
    INCR(process_metrics, ping);
    rsp->type = RSP_PONG;

    log_verb("ping req %p processed", req);
}

void
process_request(struct response *rsp, struct request *req)
{
    log_verb("processing req %p, write rsp to %p", req, rsp);
    INCR(process_metrics, process_req);

    if (req->type == REQ_PING) {
        _process_ping(rsp, req);
    } else {
        NOT_REACHED();
    }
}
