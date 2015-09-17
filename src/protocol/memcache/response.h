#pragma once

#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_queue.h>
#include <cc_util.h>

#define RSP_POOLSIZE 0

/*          name                type                default             description */
#define RESPONSE_OPTION(ACTION)                                                                 \
    ACTION( response_poolsize,  OPTION_TYPE_UINT,   str(RSP_POOLSIZE),  "response pool size"   )

/*          name                type            description */
#define RESPONSE_METRIC(ACTION)                                         \
    ACTION( response_free,      METRIC_GAUGE,   "# free rsp in pool"   )\
    ACTION( response_borrow,    METRIC_COUNTER, "# rsps borrowed"      )\
    ACTION( response_return,    METRIC_COUNTER, "# rsps returned"      )\
    ACTION( response_create,    METRIC_COUNTER, "# rsps created"       )\
    ACTION( response_destroy,   METRIC_COUNTER, "# rsps destroyed"     )

typedef struct {
    RESPONSE_METRIC(METRIC_DECLARE)
} response_metrics_st;

#define RESPONSE_METRIC_INIT(_metrics) do {                                 \
    *(_metrics) = (response_metrics_st) { RESPONSE_METRIC(METRIC_INIT) };   \
} while(0)

/**
 * Note: there are some semi special values here:
 * - a dummy entry RSP_UNKNOWN so we can use it as the initial type value;
 * - a RSP_NUMERIC type that doesn't have a corresponding message body.
 */
#define RSP_TYPE_MSG(ACTION)                        \
    ACTION( RSP_UNKNOWN,        ""                 )\
    ACTION( RSP_OK,             "OK\r\n"           )\
    ACTION( RSP_END,            "END\r\n"          )\
    ACTION( RSP_STAT,           "STAT "            )\
    ACTION( RSP_VALUE,          "VALUE "           )\
    ACTION( RSP_STORED,         "STORED\r\n"       )\
    ACTION( RSP_EXISTS,         "EXISTS\r\n"       )\
    ACTION( RSP_DELETED,        "DELETED\r\n"      )\
    ACTION( RSP_NOT_FOUND,      "NOT_FOUND\r\n"    )\
    ACTION( RSP_NOT_STORED,     "NOT_STORED\r\n"   )\
    ACTION( RSP_CLIENT_ERROR,   "CLIENT_ERROR "    )\
    ACTION( RSP_SERVER_ERROR,   "SERVER_ERROR "    )\
    ACTION( RSP_NUMERIC,        ""                 )

#define GET_TYPE(_name, _str) _name,
typedef enum response_type {
    RSP_TYPE_MSG(GET_TYPE)
    RSP_SENTINAL
} response_type_t;
#undef GET_TYPE

struct bstring rsp_strings[RSP_SENTINAL];

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


/*
 * NOTE(yao): we store fields as location in rbuf, this assumes the data will
 * not be overwritten prematurely.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct response {
    STAILQ_ENTRY(response)  next;       /* allow response pooling/chaining */
    bool                    free;

    response_state_t        rstate;     /* response state */
    response_parse_state_t  pstate;     /* parsing state */

    response_type_t         type;

    struct bstring          key;        /* key string */
    struct bstring          vstr;       /* value string */
    uint64_t                vint;       /* return value for incr/decr, or integer get value */
    uint64_t                vcas;       /* value for cas */
    struct metric           *met;       /* metric, for reporting stats */

    uint32_t                flag;
    uint32_t                vlen;

    unsigned                cas:1;      /* print cas ? */
    unsigned                num:1;      /* is the value a number? */
    unsigned                val:1;      /* value needed? */
    unsigned                error:1;    /* error */
};

void response_setup(response_metrics_st *metrics);
void response_teardown(void);

struct response *response_create(void);
void response_destroy(struct response **rsp);
void response_reset(struct response *rsp);

void response_pool_create(uint32_t max);
void response_pool_destroy(void);
struct response *response_borrow(void);
void response_return(struct response **rsp);
