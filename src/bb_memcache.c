#include <ctype.h>

#include <cc_debug.h>
#include <cc_mbuf.h>

#include <bb_memcache.h>

typedef enum request_state {
    PARSING,
    EXECUTING,
    REPLYING,
    DONE,
    RS_SENTINEL
} request_state_t;

typedef enum parse_state {
    REQ_START,
    REQ_VERB,
    REQ_POST_VERB,
} parse_state_t;

typedef enum request_verb {
    GET,
    GETS,
    SET,
    ADD,
    REPLACE,
    DELETE,
    INCR,
    DECR,
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

typedef enum token_unary {
    TOKEN_UNARY_START,
} token_unary_t;

typedef enum token_delete {
    TOKEN_START,
    TOKEN_KEY,
    TOKEN_

typedef enum retrieve_token {
    TOKEN_START,
    TOKEN_VERB,
    TOKEN_SPACE
} retrieve_token_t;

struct request {
    request_state_t state;

    request_verb_t  verb;
    request_type_t  type;

    uint32_t        flag;
    uint32_t        expiry;
    uint32_t        vlen;

    err_t           err;

    unsigned        noreply:1;
    unsigned        error:1;
    unsigned        swallow:1;
};


/* parse the first line / "header" according to memcache ASCII protocol */
void
memcache_parse_hdr(struct request *req, struct mbuf *buf)
{
    ASSERT(req != NULL);
    ASSERT(buf != NULL);


}
