#pragma once

#include <protocol/ping_include.h>

#include <cc_define.h>
#include <cc_metric.h>

/*          name                        type            description */
#define PROCESS_METRIC(ACTION)                                          \
    ACTION( process_req,       METRIC_COUNTER, "# requests processed"  )\
    ACTION( ping,              METRIC_COUNTER, "# pings processed"     )

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

#define PROCESS_METRIC_INIT(_metrics) do {                              \
    *(_metrics) = (process_metrics_st) { PROCESS_METRIC(METRIC_INIT) }; \
} while(0)

void process_setup(process_metrics_st *process_metrics);
void process_teardown(void);

void process_request(struct response *rsp, struct request *req);
