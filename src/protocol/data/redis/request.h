#pragma once

#include "constant.h"
//#include "hash.h"

#include <cc_array.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_queue.h>

#include <inttypes.h>

#define REQ_NTOKEN 127 /* # tokens in a command */
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
    ACTION( request_free,       METRIC_GAUGE,   "# free req in pool"   )\
    ACTION( request_borrow,     METRIC_COUNTER, "# reqs borrowed"      )\
    ACTION( request_return,     METRIC_COUNTER, "# reqs returned"      )\
    ACTION( request_create,     METRIC_COUNTER, "# reqs created"       )\
    ACTION( request_destroy,    METRIC_COUNTER, "# reqs destroyed"     )

typedef struct {
    REQUEST_METRIC(METRIC_DECLARE)
} request_metrics_st;

#define REQ_GROUP_MSG(ACTION)                       \
    ACTION( REQ_UNKNOWN,        ""                 )\
    ACTION( REQ_HASH,           "hash"             )\
    ACTION( REQ_ZSET,           "zset"             )

#define GET_TYPE(_name, _str) _name,
typedef enum request_group {
    REQ_GROUP_MSG(GET_TYPE)
    REQ_SENTINEL
} request_group_t;
#undef GET_TYPE

typedef enum request_state {
    REQ_PARSING,
    REQ_PARSED,
    REQ_PROCESSING,
    REQ_DONE
} request_state_t;

//struct command_storage {
//    union {
//        struct hash_command hash;
//    };
//};

struct request {
    STAILQ_ENTRY(request)   next;       /* allow request pooling/chaining */
    bool                    free;

    request_state_t         rstate;     /* request state */

    bool                    noreply;  /* skip response */
    bool                    serror;   /* server error */
    bool                    cerror;   /* client error */

    request_group_t         group;      /* so we know how to cast fields below */
//    struct command_storage  command;    /* fixed length part of the command */
    struct array            token;      /* array elements are bstrings */
    uint8_t                 data[];     /* array data are tokens in command */
    /* the memory allocated to data (pointed to by token.data) will be
     * determined by the maximum number of tokens (req_ntoken) allowed.
     */
};

void request_setup(request_options_st *options, request_metrics_st *metrics);
void request_teardown(void);

struct request *request_create(void);
void request_destroy(struct request **req);
void request_reset(struct request *req);

struct request *request_borrow(void);
void request_return(struct request **req);
