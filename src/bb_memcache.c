#include <ctype.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_util.h>

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
    REQ_SENTINEL
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

typedef enum token_retrieve {
    RETRIEVE_START,
    RETRIEVE_KEYS,
    RETRIEVE_CRLF,
    RETRIEVE_SENTINEL
} token_retrieve_t;

typedef enum token_numeric {
    NUMERIC_START,
    NUMERIC_KEY,
    NUMERIC_DELTA,
    NUMERIC_NOREPLY,
    NUMERIC_CRLF,
    NUMERIC_SENTINEL
} token_retrieve_t;

typedef enum token_storage {
    TOKEN_STORAGE_START,
    TOKEN_STORAGE_KEY,
    TOKEN_STORAGE_FLAG,
    TOKEN_STORAGE_EXPIRE,
    TOKEN_STORAGE_VLEN,
    TOKEN_STORAGE_NOREPLY,
    TOKEN_STORAGE_CRLF,
    TOKEN_STORAGE_SENTINEL
} token_retrieve_t;

struct key {
    size_t  len;
    uint8_t *data;
};

/*
 * we store key and value as location in the buffer, this assumes the data
 * will not be overwritten before the current request is completed.
 */
struct request {
    request_state_t state;
    int             token;

    request_verb_t  verb;
    request_type_t  type;

    struct key

    uint32_t        flag;
    uint32_t        expiry;
    uint32_t        vlen;

    err_t           err;

    unsigned        noreply:1;
    unsigned        error:1;
    unsigned        swallow:1;
};


/* parse unary command (post verb) */
static rstatus_t
memcache_sub_unary(struct request *req, struct mbuf *buf)
{
    uint8_t *p, *pt;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    enum token_unary {
        UNARY_START, /* a virtual token / place holder */
        UNARY_CRLF,
        UNARY_SENTINEL
    } token;

    token = (enum token_unary)req->token;
    ASSERT(token >= UNARY_START && token < UNARY_SENTINEL);

    /* unary commands are a special as it expects nothing but CRLF */
    for (p = mbuf->rpos; p < wpos; p++) {
        uint8_t ch = *p;

        switch (token) {
        case UANRY_START:
            if (ch == CR) {
                token = UNARY_CRLF;
                mbuf->rpos = p;
            } else if (ch == ' ') {
                log_debug(LOG_VERB, "unnecessary white space in unary command");
            } else {
                req->swallow = true;
                return CC_ERROR;
            }
            break;

        case UNARY_CRLF:
            if (ch == LF) {
                /* end state */
                mbuf->rpos = p + 1;
                return CC_OK;
            } else {
                req->swallow = true;
                return CC_ERROR;
            }
            break; /* will never be called, keep for uniformity */

        default:
            NOT_REACHED();
            break;
        }
    }
}

/* parse delete command (post verb) */
static rstatus_t
memcache_sub_delete(struct request *req, struct mbuf *buf)
{
    uint8_t *p, *pt;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    enum token_delete {
        DELETE_START,
        DELETE_KEY,
        DELETE_NOREPLY,
        DELETE_CRLF,
        DELETE_SENTINEL
    } token;

    token = (enum token_delete)r->token;
    ASSERT(state >= DELETE_START && state < DELETE_SENTINEL);

    /*
     * pt always points to the start of the current token.
     * rpos in mbuf is only advanced if a token is fully parsed.
     */
    pt = mbuf->rpos;
    for (p = mbuf->rpos; p < wpos; p++) {
        uint8_t ch = *p;

        switch (token) {
        case DELETE_START:
            if (ch != ' ') {
                token = DELETE_KEY;
            }
            break;

        case DELETE_KEY:
            if (ch == ' ' || ch == CR) {
            }
        case DELETE_CRLF:
            if (ch == LF) {
                mbuf->rpos = p + 1;
                return CC_OK;
            } else {
                req->swallow = true;
                return CC_ERROR;
            }
            break; /* will never be called, keep for uniformity */

        default:
            NOT_REACHED();
            break;
        }
    }

/* parse the first line / "header" according to memcache ASCII protocol */
void
memcache_parse_hdr(struct request *req, struct mbuf *buf)
{
    ASSERT(req != NULL);
    ASSERT(buf != NULL);


}
