#pragma once

#include "cmd_hash.h"
#include "cmd_misc.h"
#include "cmd_pubsub.h"
#include "cmd_zset.h"

#include <cc_array.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_queue.h>

#include <inttypes.h>

#define REQ_NTOKEN 127 /* # tokens in a command */
#define KEY_MAXLEN 255
#define REQ_POOLSIZE 0

/*          name                type                default         description */
#define REQUEST_OPTION(ACTION)                                                          \
    ACTION( request_ntoken,     OPTION_TYPE_UINT,   REQ_NTOKEN,     "# tokens in request")\
    ACTION( request_poolsize,   OPTION_TYPE_UINT,   REQ_POOLSIZE,   "request pool size")

typedef struct {
    REQUEST_OPTION(OPTION_DECLARE)
} request_options_st;

/*          name                type            description */
#define REQUEST_METRIC(ACTION)                                          \
    ACTION( request_curr,       METRIC_GAUGE,   "# req created"        )\
    ACTION( request_free,       METRIC_GAUGE,   "# free req in pool"   )\
    ACTION( request_borrow,     METRIC_COUNTER, "# reqs borrowed"      )\
    ACTION( request_return,     METRIC_COUNTER, "# reqs returned"      )\
    ACTION( request_create,     METRIC_COUNTER, "# reqs created"       )\
    ACTION( request_destroy,    METRIC_COUNTER, "# reqs destroyed"     )

typedef struct {
    REQUEST_METRIC(METRIC_DECLARE)
} request_metrics_st;

#define GET_TYPE(_type, _str, narg) _type,
typedef enum cmd_type {
    REQ_UNKNOWN,
    REQ_HASH(GET_TYPE)
    REQ_ZSET(GET_TYPE)
    REQ_MISC(GET_TYPE)
    REQ_PUBSUB(GET_TYPE)
    REQ_SENTINEL
} cmd_type_e;
#undef GET_TYPE

struct command {
    cmd_type_e      type;
    struct bstring  bstr;
    int32_t         narg;
};

extern struct command command_table[REQ_SENTINEL];

struct request {
    STAILQ_ENTRY(request)   next;     /* allow request pooling/chaining */
    bool                    free;

    bool                    noreply;  /* skip response */
    bool                    serror;   /* server error */
    bool                    cerror;   /* client error */

    cmd_type_e              type;
    struct array            *token;    /* array elements are tokens */
};

void request_setup(request_options_st *options, request_metrics_st *metrics);
void request_teardown(void);

struct request *request_create(void);
void request_destroy(struct request **req);
void request_reset(struct request *req);

struct request *request_borrow(void);
void request_return(struct request **req);
