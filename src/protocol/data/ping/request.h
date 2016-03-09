#pragma once

#include <cc_bstring.h>
#include <cc_metric.h>

/*          name                type                default         description */
#define REQUEST_OPTION(ACTION)                                                          \
    ACTION( request_poolsize,   OPTION_TYPE_UINT,   REQ_POOLSIZE,   "request pool size")

/*          name                type            description */
#define REQUEST_METRIC(ACTION)                                          \
    ACTION( request_create,     METRIC_COUNTER, "# reqs created"       )\
    ACTION( request_destroy,    METRIC_COUNTER, "# reqs destroyed"     )

typedef struct {
    REQUEST_METRIC(METRIC_DECLARE)
} request_metrics_st;

#define REQUEST_METRIC_INIT(_metrics) do {                              \
    *(_metrics) = (request_metrics_st) { REQUEST_METRIC(METRIC_INIT) }; \
} while(0)

#define REQ_TYPE_MSG(ACTION)                        \
    ACTION( REQ_UNKNOWN,        ""                 )\
    ACTION( REQ_PING,           "ping"             )

#define GET_TYPE(_name, _str) _name,
typedef enum request_type {
    REQ_TYPE_MSG(GET_TYPE)
    REQ_SENTINAL
} request_type_t;
#undef GET_TYPE

typedef enum request_state {
    REQ_PARSING,
    REQ_PARSED,
    REQ_PROCESSING,
    REQ_DONE
} request_state_t;

typedef enum request_parse_state {
    REQ_HDR
} request_parse_state_t;

struct request {
    request_state_t         rstate;     /* request state */
    request_parse_state_t   pstate;     /* parsing state */

    request_type_t          type;
};

void request_setup(request_metrics_st *metrics);
void request_teardown(void);
