#pragma once

#include <buffer/cc_dbuf.h>
#include <cc_define.h>
#include <cc_metric.h>

#include <stdint.h>

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

struct request;
struct response;

void compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp);
void compose_teardown(void);

/* if the return value is negative, it can be interpreted as compose_rstatus */
int compose_req(struct buf **buf, struct request *req);

//int compose_rsp(struct buf **buf, struct response *rsp);
