#ifndef __BB_MEMCACHE_H__
#define __BB_MEMCACHE_H__

#include <cc_define.h>
#include <cc_mbuf.h>

struct request;

typedef enum request_state {
    PARSING,
    PARSED,
    EXECUTING,
    REPLYING,
    DONE,
    RS_SENTINEL
} request_state_t;

typedef enum parse_state {
    VERB,
    POST_VERB,
    SENTINEL
} parse_state_t;

typedef enum request_verb {
    UNKNOWN,
    GET,
    GETS,
    DELETE,
    SET,
    ADD,
    REPLACE,
    CAS,
    APPEND,
    PREPEND,
    INCR,
    DECR,
    STATS,
    QUIT,
    RV_SENTINEL
} request_verb_t;

/*
 * NOTE(yao): we store key and value as location in rbuf, this assumes the data
 * will not be overwritten before the current request is completed.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct request {
    request_state_t rstate;     /* request state */
    parse_state_t   pstate;     /* parsing state */
    int             tstate;     /* token state post verb, differs by command */

    request_verb_t  verb;

    struct array    *keys;      /* elements are byte strings (in cc_string.h) */

    uint32_t        flag;
    uint32_t        expiry;
    uint32_t        vlen;
    int64_t         delta;
    uint64_t        cas;

    unsigned        noreply:1;
    unsigned        serror:1;   /* server error */
    unsigned        cerror:1;   /* client error */
    unsigned        swallow:1;  /* caused by either client or server error */
};

struct request *request_create(void);
void request_destroy(struct request *req);
void request_reset(struct request *req);
rstatus_t request_parse_hdr(struct request *req, struct mbuf *buf);

#endif
