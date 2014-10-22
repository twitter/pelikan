#ifndef _BB_REQUEST_H_
#define _BB_REQUEST_H_

#include <memcache/bb_constant.h>

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_mm.h>
#include <cc_queue.h>

#include <inttypes.h>

#define REQ_POOLSIZE 0

/*          name                type                default             description */
#define REQUEST_OPTION(ACTION)                                                              \
    ACTION( request_poolsize,   OPTION_TYPE_UINT,   str(REQ_POOLSIZE),  "request pool size")

/*          name                type            description */
#define REQPOOL_METRIC(ACTION)                                          \
    ACTION( reqpool_free,       METRIC_GAUGE,   "# free req in pool"   )\
    ACTION( reqpool_borrow,     METRIC_COUNTER, "# reqs borrowed"      )\
    ACTION( reqpool_return,     METRIC_COUNTER, "# reqs returned"      )\
    ACTION( reqpool_create,     METRIC_COUNTER, "# reqs created"       )\
    ACTION( reqpool_destroy,    METRIC_COUNTER, "# reqs destroyed"     )

typedef enum request_state {
    PARSING,
    PARSED,
    PROCESSING,
    DONE,
    RS_SENTINEL
} request_state_t;

typedef enum parse_state {
    REQ_HDR,
    REQ_VAL,
    PS_SENTINEL
} parse_state_t;

typedef enum request_verb {
    REQ_UNKNOWN,
    REQ_GET,
    REQ_GETS,
    REQ_DELETE,
    REQ_SET,
    REQ_ADD,
    REQ_REPLACE,
    REQ_CAS,
    REQ_APPEND,
    REQ_PREPEND,
    REQ_INCR,
    REQ_DECR,
    REQ_STATS,
    REQ_QUIT,
    RV_SENTINEL
} request_verb_t;

/*
 * NOTE(yao): we store key and value as location in rbuf, this assumes the data
 * will not be overwritten before the current request is completed.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct request {
    STAILQ_ENTRY(request)   next;       /* allow request pooling */

    request_state_t         rstate;     /* request state */
    parse_state_t           pstate;     /* parsing state */
    int                     tstate;     /* token state post verb */

    request_verb_t          verb;

    struct array            *keys;      /* elements are bstrings */
    struct bstring          vstr;       /* the value string */

    uint32_t                flag;
    uint32_t                expiry;
    uint32_t                vlen;
    uint64_t                delta;
    uint64_t                cas;

    unsigned                noreply:1;
    unsigned                serror:1;   /* server error */
    unsigned                cerror:1;   /* client error */
    unsigned                swallow:1;  /* caused by errors */
};

struct request *request_create(void);
void request_destroy(struct request *req);
void request_reset(struct request *req);

void request_pool_create(uint32_t max);
void request_pool_destroy(void);
struct request *request_borrow(void);
void request_return(struct request *req);

#endif /* _BB_REQUEST_H_ */
