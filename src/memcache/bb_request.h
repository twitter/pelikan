#ifndef __BB_MEMCACHE_H__
#define __BB_MEMCACHE_H__

#include <cc_mbuf.h>

#define MAX_KEY_LEN 250

struct request;

typedef enum request_state {
    PARSING,
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
    SET,
    ADD,
    REPLACE,
    DELETE,
    CAS,
    INCR,
    DECR,
    APPEND,
    PREPEND,
    STATS,
    QUIT,
    RV_SENTINEL
} request_verb_t;

typedef enum request_type {
    UNARY,
    DELETE,
    RETRIEVE,
    STORE,
    ARITHMETIC,
    RT_SENTINEL
} request_type_t;

struct token {
    uint32_t len; /* size of the key */
    uint8_t *pos; /* start position of the key (in rbuf) */
};


/*
 * NOTE(yao): we store key and value as location in rbuf, this assumes the data
 * will not be overwritten before the current request is completed.
 * Whether this is a reasonable design decision eventually remains to be seen.
 */
struct request {
    request_state_t rstate;     /* request state */
    request_parse_t pstate;     /* parsing state */
    int             tstate;     /* token state */

    request_verb_t  verb;
    request_type_t  type;

    struct array    *keys;      /* element is of struct token type */

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

void request_init(struct request *req);
rstatus_t request_reset(struct request *req);
rstatus_t request_parse_hdr(struct request *req, struct mbuf *buf);

#endif
