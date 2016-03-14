#pragma once

#include <protocol/data/memcache/constant.h>

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_mm.h>
#include <cc_option.h>
#include <cc_queue.h>

#include <inttypes.h>

#define REQ_POOLSIZE 4096

/*          name                type                default         description */
#define REQUEST_OPTION(ACTION)                                                          \
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

#define REQUEST_METRIC_INIT(_metrics) do {                              \
    *(_metrics) = (request_metrics_st) { REQUEST_METRIC(METRIC_INIT) }; \
} while(0)

#define REQ_TYPE_MSG(ACTION)                        \
    ACTION( REQ_UNKNOWN,        ""                 )\
    ACTION( REQ_GET,            "get"              )\
    ACTION( REQ_GETS,           "gets"             )\
    ACTION( REQ_DELETE,         "delete "          )\
    ACTION( REQ_SET,            "set "             )\
    ACTION( REQ_ADD,            "add "             )\
    ACTION( REQ_REPLACE,        "replace "         )\
    ACTION( REQ_CAS,            "cas "             )\
    ACTION( REQ_APPEND,         "append "          )\
    ACTION( REQ_PREPEND,        "prepend "         )\
    ACTION( REQ_INCR,           "incr "            )\
    ACTION( REQ_DECR,           "decr "            )\
    ACTION( REQ_FLUSH,          "flush_all\r\n"    )\
    ACTION( REQ_QUIT,           "quit\r\n"         )\

#define GET_TYPE(_name, _str) _name,
typedef enum request_type {
    REQ_TYPE_MSG(GET_TYPE)
    REQ_SENTINEL
} request_type_t;
#undef GET_TYPE

extern struct bstring req_strings[REQ_SENTINEL];

typedef enum request_state {
    REQ_PARSING,
    REQ_PARSED,
    REQ_PROCESSING,
    REQ_DONE
} request_state_t;

typedef enum request_parse_state {
    REQ_HDR,
    REQ_VAL
} request_parse_state_t;

/*
 * NOTE(yao): we store key and value as location in rbuf, this assumes the data
 * will not be overwritten before the current request is completed.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct request {
    STAILQ_ENTRY(request)   next;       /* allow request pooling */
    bool                    free;

    request_state_t         rstate;     /* request state */
    request_parse_state_t   pstate;     /* parsing state */

    request_type_t          type;

    struct array            *keys;      /* elements are bstrings */
    struct bstring          vstr;       /* the value string */
    uint32_t                nfound;     /* number of keys found */

    uint32_t                flag;
    uint32_t                expiry;
    uint32_t                vlen;
    uint64_t                delta;
    uint64_t                vcas;

    unsigned                noreply:1;
    unsigned                val:1;      /* value needed? */
    unsigned                serror:1;   /* server error */
    unsigned                cerror:1;   /* client error */
};

void request_setup(request_options_st *options, request_metrics_st *metrics);
void request_teardown(void);

struct request *request_create(void);
void request_destroy(struct request **req);
void request_reset(struct request *req);

struct request *request_borrow(void);
void request_return(struct request **req);
