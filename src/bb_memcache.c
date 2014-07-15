#include <ctype.h>

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_util.h>

#include <bb_memcache.h>

#define MAX_TOKEN_LEN 256
#define MAX_BATCH_SIZE 50

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

typedef enum token_retrieve {
    RETRIEVE_START,
    RETRIEVE_KEYS,
    RETRIEVE_CRLF,
    RETRIEVE_SENTINEL
} token_retrieve_t;

struct token {
    uint32_t len; /* size of the key */
    uint8_t *pos; /* start position of the key (in rbuf) */
};


/*
 * NOTE(yao): we store key and value as location in rbuf, this assumes the data
 * will not be overwritten before the current request is completed.
 * Whether this is a reasonable design decision or not remains to be seen.
 */
struct request {
    request_state_t rstate;     /* request state */
    request_verb_t  verb;
    request_type_t  type;

    int             tstate;     /* token state */

    struct array    keys;       /* element is of struct token type */

    uint32_t        flag;
    uint32_t        expiry;
    uint32_t        vlen;
    int64_t         delta;

    unsigned        noreply:1;
    unsigned        serror:1;   /* server error */
    unsigned        cerror:1;   /* client error */
    unsigned        swallow:1;  /* caused by either client or server error */

    err_t           err;
};

static inline void
_memcache_mark_serror(struct request *req, struct mbuf *buf, uint8_t *npos)
{
    /*
     * NOTE(yao): some server errors can actually be rescued internally, such
     * as by retrying. For simplicity, we simply abort the request for now
     */
    req->swallow = true;
    req->serror = true;

    buf->rpos = npos;
}

static inline void
_memcache_mark_cerror(struct request *req, struct mbuf *buf, uint8_t *npos)
{
    /*
     * NOTE(yao): swallow always runs to the next CRLF, so if we set npos to be
     * after the current one, we run the risk of swallowing another request that
     * might have been totally legit.
     * Therefore, call this cerror with the new position right at or before CRLF
     */
    req->swallow = true;
    req->cerror = true;

    buf->rpos = npos;
}

/* NOTE(yao): if token proves useful outside of the parser we should move it out */
static inline void
_memcache_token_init(struct token *t)
{
    t->len = 0;
    t->pos = NULL;
}

static inline void
_memcache_token_start(struct token *t, uint8_t *p)
{
    t->pos = p;
    t->len = 1;
}

static inline rstatus_t
_memcache_token_check_size(struct request req, struct mbuf *buf, uint8_t *p)
{
    /* TODO(yao): allow caller to provide token size limit for each field*/
    if (p - buf->rpos >= MAX_TOKEN_LEN) {
        log_warn("ill formatted request: token size exceeds %zu",
                MAX_TOKEN_LEN);

        _memcache_mark_cerror(req, buf, p);

        return CC_ERROR;
    }

    return CC_OK;
}

/* CRLF is special and we need to "peek into the future" */
static inline rstatus_t
_memcache_try_crlf(struct mbuf *buf, uint8_t *p)
{
    if (*p != CR) {
        return CC_ERROR;
    }

    if (buf->wpos == p + 1) {
        return CC_UNFIN;
    }

    if (*(p + 1) == LF) {
        return CC_OK;
    } else {
        return CC_ERROR;
    }
}


static rstatus_t
_memcache_chase_crlf(struct request *req, struct mbuf *buf)
{
    uint8_t *p;
    rstatus_t status;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    for (p = mbuf->rpos; p < wpos; p++) {
        status = _memcache_token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _memcache_try_crlf(buf, p);
        switch (status) {
        case CC_UNFIN:
            return CC_UNFIN;

        case CC_ERROR: /* not CRLF */
            if (*p != ' ') {
                _memcache_mark_cerror(req, buf, p + 1);

                log_warn("ill formatted request: illegal character");

                return CC_ERROR;
            } else {
                log_debug(LOG_VERB, "unnecessary whitespace");
            }

            break;

        case CC_OK:
            buf->rpos = p + CRLF_LEN;

            return CC_OK;

        default:
            NOT_REACHED();
            break;
        }
    }

    /* there isn't enough data in buf to fully parse the request*/
    return CC_UNFIN;
}


/*
 * NOTE(yao): In the following parser/subparser functions, we move the rpos
 * pointer in mbuf forward when we finish parsing a token fully.
 */

/* parse unary command (post verb) */
/* unary commands are a special as it expects nothing but CRLF */
static rstatus_t
memcache_sub_unary(struct request *req, struct mbuf *buf)
{
    return _memcache_chase_crlf(req, buf);
}


static inline rstatus_t
_memcache_check_key(struct request *req, struct mbuf *buf, bool *end,
        struct token *t, uint8_t *p)
{
    rstatus_t status;
    struct token *k; /* a key token */
    bool complete = false;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return CC_UNFIN;
    }

    if (*p == ' ') {
        complete = true;
        *end = false;
    } else {
        status = _memcache_try_crlf(buf, p);
        if (status == CC_OK) {
            if (t->len == 0) {
                log_warn("ill formatted request: no key provided");

                goto key_cerror;
            }

            if (!*end) {
                log_warn("ill formatted request: missing field(s)");

                goto key_cerror;
            } else {
                complete = true;
            }
        }
    }

    if (complete) {
        status = _memcache_push_key(req, t);
        k = array_push(req->keys);
        if (k == NULL) {
            log_warn("push request key failed: no memory");

            _memcache_mark_serror(req, buf, p + *end ? CRLF_LEN : 1);

            return CC_NOMEM;
        }

        k->pos = t->pos;
        k->len = t->len;
        buf->rpos = p + *end ? CRLF_LEN : 1;

        return CC_OK;
    }

    /* the current character is part of the key */
    if (t->len == 0) {
        _memcache_token_start(t, p);
    } else {
        t->len++;
    }

    return CC_UNFIN;

error:
    _memcache_mark_cerror(req, buf, p);

    return CC_ERROR;
}


static rstatus_t
_memcache_chase_key(struct request *req, struct mbuf *buf, bool *end)
{
    uint8_t *p;
    rstatus_t status;
    struct token t;

    _memcache_token_init(&t);
    for (p = mbuf->rpos; p < wpos; p++) {
        status = _memcache_token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _memcache_check_key(req, buf, end, &t, p);
        switch (status) {
        case CC_UNFIN:
            break;

        case CC_OK:
        case CC_ERROR:
        case CC_NOMEM:
            return status;

        default:
            NOT_REACHED();
            break;
        }
    }

    return CC_UNFIN;
}


static inline rstatus_t
_memcache_check_noreply(struct request *req, struct mbuf *buf, bool *end,
        struct token *t, uint8_t *p)
{
    bool complete = false;
    /* *end should always be true according to the protocol */

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return CC_UNFIN;
    }

    if (*p == ' ') {
        complete = true;
        *end = false;
    } else {
        status = _memcache_try_crlf(buf, p);
        if (status == CC_OK) {
            complete = true;

            if (t->len == 0) {
                buf->rpos = p + CRLF_LEN;

                return CC_OK;
            }
        }
    }

    if (complete) {
        if (t->len == 7 && str7cmp(t->pos, 'n', 'o', 'r', 'e', 'p', 'l', 'y')) {
            req->noreply = 1;
            buf->rpos = p + *end ? CRLF_LEN : 1;

            return CC_OK;
        } else {
            _memcache_mark_cerror(req, buf, p + *end ? CRLF_LEN : 1);

            return CC_ERROR;
        }
    }

    if (t->len == 0) {
        _memcache_token_start(t, p);
    } else {
        t->len++;
    }

    return CC_UNFIN;
}


static rstatus_t
_memcache_chase_noreply(struct request *req, struct mbuf *buf, bool *end)
{
    uint8_t *p;
    rstatus_t status;
    struct token t;

    _memcache_token_init(&t);
    for (p = mbuf->rpos; p < wpos; p++) {
        status = _memcache_token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _memcache_check_noreply(req, buf, end, &t, p);
        switch (status) {
        case CC_UNFIN:
            break;

        case CC_OK:
        case CC_ERROR:
            return status;

        default:
            NOT_REACHED();
            break;
        }
    }

    return CC_UNFIN;
}


static rstatus_t
memcache_delete(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    bool end;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    enum token_delete {
        DELETE_KEY = 0,
        DELETE_NOREPLY,
        DELETE_CRLF,
        DELETE_SENTINEL
    } tstate;

    tstate = (enum token_delete)req->tstate;
    ASSERT(tstate >= DELETE_KEY && tstate < DELETE_SENTINEL);

    switch (tstate) {
    case DELETE_KEY:
        end = true;
        status = _memcache_chase_key(req, buf, &end);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = DELETE_NOREPLY;

    case DELETE_NOREPLY: /* fall-through intended */
        end = true;
        status = _memcache_chase_noreply(req, buf, &end);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = DELETE_CRLF;

    case DELETE_CRLF: /* fall-through intended */
        return _memcache_chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }
}


static inline rstatus_t
_memcache_check_uint(uint64_t *num, struct request *req, struct mbuf *buf,
        bool *end, struct token *t, uint8_t *p, uint64_t max)
{
    bool complete = false;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return CC_UNFIN;
    }

    if (*p == ' ') {
        complete = true;
        *end = false;
    } else {
        status = _memcache_try_crlf(buf, p);
        if (status == CC_OK) {
            if (t->len == 0) {
                log_warn("ill formatted request: no integer provided");

                goto error;
            }

            if (!*end) {
                log_warn("ill formatted request: missing field(s)");

                goto error;
            } else {
                complete = true;
            }
        }
    }

    if (complete) {
        return CC_OK;
    }

    if (isdigit(*p)) {
        if (*num > max / 10) {
            /* TODO(yao): catch the few numbers that will still overflow */

            log_warn("ill formatted request: integer too big");

            goto error;
        }

        t->len++;
        *num = *num * 10ULL + (uint64_t)(*p - '0');

        return CC_UNFIN;
    } else {
        log_warn("ill formatted request: non-digit char in integer field");

        goto error;
    }

    return CC_UNFIN;

error:
    _memcache_mark_cerror(req, buf, p);

    return CC_ERROR;
}


static rstatus_t
_memcache_chase_uint(uint64_t *num, struct request *req, struct mbuf *buf,
        bool *end, uint64_t max)
{
    uint8_t *p;
    rstatus_t status;
    struct token t;

    *num = 0;
    _memcache_token_init(&t);
    for (p = mbuf->rpos; p < wpos; p++) {
        status = _memcache_token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _memcache_check_uint(num, req, buf, &t, end, p, max);
        switch (status) {
        case CC_UNFIN:
            break;

        case CC_OK:
        case CC_ERROR: /* fall-through intended */
            return status;

        default:
            NOT_REACHED();
            break;
        }
    }

    return CC_UNFIN;
}


static rstatus_t
memcache_arithmetic(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    uint64_t delta;
    bool end;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    enum token_numeric {
        NUMERIC_KEY = 0,
        NUMERIC_DELTA,
        NUMERIC_NOREPLY,
        NUMERIC_CRLF,
        NUMERIC_SENTINEL
    } tstate;

    tstate = (enum token_numeric)req->tstate;
    ASSERT(tstate >= NUMERIC_KEY && tstate < NUMERIC_SENTINEL);

    switch (tstate) {
    case NUMERIC_KEY:
        end = false;
        status = _memcache_chase_key(req, buf, &end);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = NUMERIC_DELTA;

    case NUMERIC_DELTA: /* fall-through intended */
        end = true;
        delta = 0;
        status = _memcache_chase_uint(&delta, req, buf, &end, INT64_MAX);
        if (status== CC_OK) {
            req->delta = (int64_t)delta;
        }
        if (status != CC_OK || *end) {
            return status;
        }

        req->tstate = NUMERIC_NOREPLY;

    case NUMERIC_NOREPLY: /* fall-through intended */
        end = true;
        status = _memcache_chase_noreply(req, buf, &end);
        if (status != CC_OK || *end) {
            return status;
        }

        req->tstate = NUMERIC_CRLF;

    case NUMERIC_CRLF: /* fall-through intended */
        return _memcache_chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }
}


static rstatus_t
memcache_store(struct request *req, struct mbuf *buf)
{
    uint8_t *p;
    rstatus_t status;
    uint64_t num;
    bool end;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    enum token_store {
        STORE_KEY = 0,
        STORE_FLAG,
        STORE_EXPIRE,
        STORE_VLEN,
        STORE_NOREPLY,
        STORE_CRLF,
        STORE_SENTINEL
    } tstate;

    tstate = (enum token_store)req->tstate;
    ASSERT(tstate >= STORE_KEY && tstate < STORE_SENTINEL);

    switch (tstate) {
    case STORE_KEY:
        end = false;
        status = _memcache_chase_key(req, buf, &end);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = STORE_FLAG;

    case STORE_FLAG: /* fall-through intended */
        end = false;
        num = 0;
        status = _memcache_chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->flag = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = STORE_EXPIRE;

    case STORE_EXPIRE: /* fall-through intended */
        end = false;
        num = 0;
        status = _memcache_chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->flag = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = STORE_VLEN;

    case STORE_VLEN: /* fall-through intended */
        end = true;
        num = 0;
        status = _memcache_chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->vlen = (uint32_t)num;
        }
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = STORE_NOREPLY;

    case STORE_NOREPLY: /* fall-through intended */
        end = true;
        status = _memcache_chase_noreply(req, buf, &end);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = STORE_CRLF;

    case STORE_CRLF: /* fall-through intended */
        return _memcache_chase_crlf(req, buf);

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
