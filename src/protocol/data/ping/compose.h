#pragma once

#include <protocol/data/ping/request.h>
#include <protocol/data/ping/response.h>

#include <cc_metric.h>

/*          name                    Type            description */
#define COMPOSE_REQ_METRIC(ACTION)                                          \
    ACTION( request_compose,        METRIC_COUNTER, "# requests composed"  )\
    ACTION( request_compose_ex,     METRIC_COUNTER, "# composing error"    )

/*          name                    Type            description */
#define COMPOSE_RSP_METRIC(ACTION)                                          \
    ACTION( response_compose,       METRIC_COUNTER, "# responses composed" )\
    ACTION( response_compose_ex,    METRIC_COUNTER, "# rsp composing error")

typedef struct {
    COMPOSE_REQ_METRIC(METRIC_DECLARE)
} compose_req_metrics_st;

typedef struct {
    COMPOSE_RSP_METRIC(METRIC_DECLARE)
} compose_rsp_metrics_st;

typedef enum compose_rstatus {
    COMPOSE_OK          = 0,
    COMPOSE_EUNFIN      = -1,
    COMPOSE_ENOMEM      = -2,
    COMPOSE_EINVALID    = -3,
    COMPOSE_EOTHER      = -4,
} compose_rstatus_t;

void compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp);
void compose_teardown(void);
