#pragma once

#include <buffer/cc_buf.h>
#include <cc_define.h>
#include <cc_metric.h>

#include <stdint.h>

/* Note(yao): the prefix cmd_ is mostly to be compatible with Twemcache metric
 * names.
 * On the other hand, the choice of putting request in front of parse instead of
 * the other way around in `request_parse' is to allow users to easily query all
 * metrics related to requests , similar for responses.
 */
/*          name                type            description */
#define PARSE_REQ_METRIC(ACTION)                                        \
    ACTION( request_parse,      METRIC_COUNTER, "# requests parsed"    )\
    ACTION( request_parse_ex,   METRIC_COUNTER, "# parsing error"      )

/*          name                type            description */
#define PARSE_RSP_METRIC(ACTION)                                        \
    ACTION( response_parse,     METRIC_COUNTER, "# responses parsed"   )\
    ACTION( response_parse_ex,  METRIC_COUNTER, "# rsp parsing error"  )\

typedef struct {
    PARSE_REQ_METRIC(METRIC_DECLARE)
} parse_req_metrics_st;

typedef struct {
    PARSE_RSP_METRIC(METRIC_DECLARE)
} parse_rsp_metrics_st;

typedef enum parse_rstatus {
    PARSE_OK        = 0,
    PARSE_EUNFIN    = -1,
    PARSE_EEMPTY    = -2,
    PARSE_EOVERSIZE = -3,
    PARSE_EINVALID  = -4,
    PARSE_EOTHER    = -5,
} parse_rstatus_e;

struct request;
struct response;

void parse_setup(parse_req_metrics_st *req, parse_rsp_metrics_st *rsp);
void parse_teardown(void);

parse_rstatus_e parse_req(struct request *req, struct buf *buf);

parse_rstatus_e parse_rsp(struct response *rsp, struct buf *buf);
