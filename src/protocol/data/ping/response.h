#pragma once

#include <cc_bstring.h>
#include <cc_metric.h>

/*          name                type                default         description */
#define RESPONSE_OPTION(ACTION)                                                             \
    ACTION( response_poolsize,  OPTION_TYPE_UINT,   RSP_POOLSIZE,   "response pool size"   )

/*          name                type            description */
#define RESPONSE_METRIC(ACTION)                                         \
    ACTION( response_create,    METRIC_COUNTER, "# rsps created"       )\
    ACTION( response_destroy,   METRIC_COUNTER, "# rsps destroyed"     )

typedef struct {
    RESPONSE_METRIC(METRIC_DECLARE)
} response_metrics_st;

#define RESPONSE_METRIC_INIT(_metrics) do {                                 \
    *(_metrics) = (response_metrics_st) { RESPONSE_METRIC(METRIC_INIT) };   \
} while(0)

#define RSP_TYPE_MSG(ACTION)                        \
    ACTION( RSP_UNKNOWN,        ""                 )\
    ACTION( RSP_PONG,           "PONG\r\n"         )

#define GET_TYPE(_name, _str) _name,
typedef enum response_type {
    RSP_TYPE_MSG(GET_TYPE)
    RSP_SENTINAL
} response_type_t;
#undef GET_TYPE

typedef enum response_state {
    RSP_PARSING,
    RSP_PARSED,
    RSP_PROCESSING,
    RSP_DONE
} response_state_t;

typedef enum response_parse_state {
    RSP_HDR,
    RSP_VAL
} response_parse_state_t;


struct response {
    response_state_t        rstate;     /* response state */
    response_parse_state_t  pstate;     /* parsing state */

    response_type_t         type;
};

void response_setup(response_metrics_st *metrics);
void response_teardown(void);
