#pragma once

#include "token.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_queue.h>
#include <cc_util.h>

#define RSP_NTOKEN 127 /* # tokens in a response */
#define RSP_POOLSIZE 0

/*          name                type                default         description */
#define RESPONSE_OPTION(ACTION)                                                             \
    ACTION( response_ntoken,    OPTION_TYPE_UINT,   RSP_NTOKEN,     "# tokens in response" )\
    ACTION( response_poolsize,  OPTION_TYPE_UINT,   RSP_POOLSIZE,   "response pool size"   )

typedef struct {
    RESPONSE_OPTION(OPTION_DECLARE)
} response_options_st;

/*          name                type            description */
#define RESPONSE_METRIC(ACTION)                                         \
    ACTION( response_curr,      METRIC_GAUGE,   "# rsp created"        )\
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
#define RSP_OK "OK"
#define RSP_NOTFOUND "NOT_FOUND"
#define RSP_PONG "PONG"
#define RSP_EXIST "EXIST" /* key already exists and op is non-overwriting */
#define RSP_NOOP "NOOP" /* key unmodified */

#define RSP_ERR_ARG "Err invalid argument"
#define RSP_ERR_NOSUPPORT "Err command not supported"
#define RSP_ERR_OUTOFRANGE "Err index out of range"
#define RSP_ERR_SERVER "Err unspecified server failure"
#define RSP_ERR_STORAGE "Err storage failure"
#define RSP_ERR_TYPE "Err type mismatch"

/*
 * NOTE(yao): we store fields as location in rbuf, this assumes the data will
 * not be overwritten prematurely.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */

struct response {
    STAILQ_ENTRY(response)  next;   /* allow response pooling/chaining */
    bool                    free;

    bool                    serror; /* server error */

    element_type_e          type;   /* only array can have >1 token */
    bool                    nil;    /* null array or null bulk string */
    struct array            *token; /* member type: `struct element' */
    uint32_t                offset; /* location of first non-attribute token */

    /* global attributes */
    uint32_t                ttl;
    uint32_t                soft_ttl;
};

void response_setup(response_options_st *options, response_metrics_st *metrics);
void response_teardown(void);

struct response *response_create(void);
void response_destroy(struct response **rsp);
void response_reset(struct response *rsp);

struct response *response_borrow(void);
void response_return(struct response **rsp);
