#pragma once

#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_queue.h>
#include <cc_util.h>

#define RSP_POOLSIZE 0

/*          name                type                default         description */
#define RESPONSE_OPTION(ACTION)                                                             \
    ACTION( response_poolsize,  OPTION_TYPE_UINT,   RSP_POOLSIZE,   "response pool size"   )

typedef struct {
    RESPONSE_OPTION(OPTION_DECLARE)
} response_options_st;

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
    RSP_SENTINEL
} response_type_t;
#undef GET_TYPE

extern struct bstring rsp_strings[RSP_SENTINEL];

typedef enum response_state {
    RSP_PARSING,
    RSP_PARSED,
    RSP_PROCESSING,
    RSP_DONE
} response_state_t;

#define RSP_VAL_BUF_SIZE 1048576

/*
 * NOTE(yao): we store fields as location in rbuf, this assumes the data will
 * not be overwritten prematurely.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct response {
    STAILQ_ENTRY(response)  next;       /* allow response pooling/chaining */
    bool                    free;

    response_state_t        rstate;     /* response state */

    response_type_t         type;

    struct bstring          key;        /* key string */
    struct bstring          vstr;       /* value string */
    char                    *vbuf;      /* vbuf is a buffer of RSP_VAL_BUF_SIZE that processors can use by
                                         * rsp->vstr.data = rsp->vbuf. vstr.data is nulled out in response_reset
                                         * so the link is broken after each response */

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

void response_setup(response_options_st *options, response_metrics_st *metrics);
void response_teardown(void);

struct response *response_create(void);
void response_destroy(struct response **rsp);
void response_reset(struct response *rsp);

struct response *response_borrow(void);
void response_return(struct response **rsp);
void response_return_all(struct response **rsp); /* return all responses in chain */
