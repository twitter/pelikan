#include <ctype.h>

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_mm.h>
#include <cc_util.h>

#include <memcache/bb_request.h>

#define MAX_TOKEN_LEN 256
#define MAX_BATCH_SIZE 50

typedef rstatus_t (*check_token_t)(struct request *req, struct mbuf *buf,
        bool *end, struct token *t, uint8_t *p);

static inline void
_mark_serror(struct request *req, struct mbuf *buf, uint8_t *npos)
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
_mark_cerror(struct request *req, struct mbuf *buf, uint8_t *npos)
{
    /*
     * NOTE(yao): swallow always runs to the next CRLF, so if we set npos to be
     * after the current one, we run the risk of swallowing another request that
     * might have been totally legit.
     * Therefore, call this cerror without skipping the current CRLF
     */
    req->swallow = true;
    req->cerror = true;

    buf->rpos = npos;
}

/* NOTE(yao): if token proves useful outside of the parser we should move it out */
static inline void
_token_init(struct token *t)
{
    t->len = 0;
    t->pos = NULL;
}

static inline void
_token_start(struct token *t, uint8_t *p)
{
    t->pos = p;
    t->len = 1;
}

/*
 * NOTE(yao): In the following parser/subparser functions, we move the rpos
 * pointer in mbuf forward when we finish parsing a token fully. This simplifies
 * the state machine.
 */

static inline rstatus_t
_token_check_size(struct request req, struct mbuf *buf, uint8_t *p)
{
    /* TODO(yao): allow caller to provide token size limit for each field*/
    if (p - buf->rpos >= MAX_TOKEN_LEN) {
        log_warn("ill formatted request: token size exceeds %zu",
                MAX_TOKEN_LEN);

        _mark_cerror(req, buf, p);

        return CC_ERROR;
    }

    return CC_OK;
}

/* CRLF is special and we need to "peek into the future" */
static inline rstatus_t
_try_crlf(struct mbuf *buf, uint8_t *p)
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
_chase_crlf(struct request *req, struct mbuf *buf)
{
    uint8_t *p;
    rstatus_t status;

    for (p = mbuf->rpos; p < wpos; p++) {
        status = _token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _try_crlf(buf, p);
        switch (status) {
        case CC_UNFIN:
            return CC_UNFIN;

        case CC_ERROR: /* not CRLF */
            if (*p != ' ') {
                _mark_cerror(req, buf, p);

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


static inline rstatus_t
_check_key(struct request *req, struct mbuf *buf, bool *end,
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
        status = _try_crlf(buf, p);
        if (status == CC_OK) {
            if (t->len == 0) {
                if (array_nelem(req->keys) == 0) {
                    log_warn("ill formatted request: no key provided");

                    goto error;
                } else {
                    /* we don't have to check *end here because the only case
                     * where this function is called when a key already exists
                     * is for multi-get.
                     */
                    return CC_OK;
                }
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
        if (array_nelem(req->keys) >= MAX_BATCH_SIZE) {
            log_warn("ill formatted request: too many keys in a batch");

            goto error;
        }

        k = array_push(req->keys);
        /* push should never fail as keys are preallocated for MAX_BATCH_SIZE */

        k->pos = t->pos;
        k->len = t->len;
        buf->rpos = p + *end ? CRLF_LEN : 1;

        return CC_OK;
    }

    /* the current character is part of the key */
    if (t->len == 0) {
        _token_start(t, p);
    } else {
        t->len++;
    }

    return CC_UNFIN;

error:
    _mark_cerror(req, buf, p);

    return CC_ERROR;
}


static inline rstatus_t
_check_verb(struct request *req, struct mbuf *buf, bool *end, struct token *t, uint8_t *p)
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
        status = _try_crlf(buf, p);
        if (status == CC_OK) {
            if (t->len == 0) {
                log_warn("ill formatted request: empty request");

                goto error;
            }

            complete = true;
        }
    }

    if (complete) {
        ASSERT(req->verb == UNKNOWN);

        switch (p - t->pos) {
        case 3:
            if (str3cmp(m, 'g', 'e', 't')) {
                req->verb = GET;
                break;
            }

            if (str3cmp(m, 's', 'e', 't')) {
                req->verb = SET;
                break;
            }

            if (str3cmp(m, 'a', 'd', 'd')) {
                req->verb = ADD;
                break;
            }

            if (str3cmp(m, 'c', 'a', 's')) {
                req->verb = ADD;
                break;
            }

            break;

        case 4:
            if (str4cmp(m, 'g', 'e', 't', 's')) {
                req->verb = GETS;
                break;
            }

            if (str4cmp(m, 'i', 'n', 'c', 'r')) {
                req->verb = INCR;
                break;
            }

            if (str4cmp(m, 'd', 'e', 'c', 'r')) {
                req->verb = DECR;
                break;
            }

            if (str4cmp(m, 'q', 'u', 'i', 't')) {
                req->verb = QUIT;
                break;
            }

            break;

        case 5:
            if (str5cmp(m, 's', 't', 'a', 't', 's')) {
                req->verb = STATS;
                break;
            }

            break;

        case 6:
            if (str6cmp(m, 'd', 'e', 'l', 'e', 't', 'e')) {
                req->verb = DELETE;
                break;
            }

            if (str6cmp(m, 'a', 'p', 'p', 'e', 'n', 'd')) {
                req->verb = APPEND;
                break;
            }

            break;

        case 7:
            if (str7cmp(m, 'r', 'e', 'p', 'l', 'a', 'c', 'e')) {
                req->verb = REPLACE;
                break;
            }

            if (str7cmp(m, 'p', 'r', 'e', 'p', 'e', 'n', 'd')) {
                req->verb = PREPEND;
                break;
            }

            break;
        }

        if (req->verb == UNKNOWN) { /* no match */
            _mark_cerror(req, buf, p);

            return CC_ERROR;
        } else {
            buf->rpos = p + *end ? CRLF_LEN : 1;

            return CC_OK;
        }
    }


    if (t->len == 0) {
        _token_start(t, p);
    } else {
        t->len++;
    }

    return CC_UNFIN;
}


static inline rstatus_t
_check_noreply(struct request *req, struct mbuf *buf, bool *end, struct token *t, uint8_t *p)
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
        status = _try_crlf(buf, p);
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
            _mark_cerror(req, buf, p);

            return CC_ERROR;
        }
    }

    if (t->len == 0) {
        _token_start(t, p);
    } else {
        t->len++;
    }

    return CC_UNFIN;
}


static rstatus_t
_chase_string(struct request *req, struct mbuf *buf, bool *end, check_token_t checker)
{
    uint8_t *p;
    rstatus_t status;
    struct token t;

    _token_init(&t);
    for (p = mbuf->rpos; p < wpos; p++) {
        status = _token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = checker(req, buf, end, &t, p);
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


static inline rstatus_t
_check_uint(uint64_t *num, struct request *req, struct mbuf *buf, bool *end,
        struct token *t, uint8_t *p, uint64_t max)
{
    bool complete = false;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return CC_UNFIN;
    }

    if (*p == ' ') {
        complete = true;
        *end = false;
    } else {
        status = _try_crlf(buf, p);
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
    _mark_cerror(req, buf, p);

    return CC_ERROR;
}


static rstatus_t
_chase_uint(uint64_t *num, struct request *req, struct mbuf *buf, bool *end,
        uint64_t max)
{
    uint8_t *p;
    rstatus_t status;
    struct token t;

    *num = 0;
    _token_init(&t);
    for (p = mbuf->rpos; p < wpos; p++) {
        status = _token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _check_uint(num, req, buf, &t, end, p, max);
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
_subrequest_delete(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    bool end;

    enum token_delete {
        KEY = 0,
        NOREPLY,
        CRLF,
        SENTINEL
    } tstate;

    tstate = (enum token_delete)req->tstate;
    ASSERT(tstate >= KEY && tstate < SENTINEL);

    switch (tstate) {
    case KEY:
        end = true;
        status = _chase_string(req, buf, &end, &_check_key);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = NOREPLY;

    case NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, &_check_noreply);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = CRLF;

    case CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }
}


static rstatus_t
_subrequest_arithmetic(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    uint64_t delta;
    bool end;

    enum token_arithmetic {
        KEY = 0,
        DELTA,
        NOREPLY,
        CRLF,
        SENTINEL
    } tstate;

    tstate = (enum token_arithmetic)req->tstate;
    ASSERT(tstate >= KEY && tstate < SENTINEL);

    switch (tstate) {
    case KEY:
        end = false;
        status = _chase_string(req, buf, &end, &_check_key);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = DELTA;

    case DELTA: /* fall-through intended */
        end = true;
        delta = 0;
        status = _chase_uint(&delta, req, buf, &end, INT64_MAX);
        if (status== CC_OK) {
            req->delta = (int64_t)delta;
        }
        if (status != CC_OK || *end) {
            return status;
        }

        req->tstate = NOREPLY;

    case NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, &_check_noreply);
        if (status != CC_OK || *end) {
            return status;
        }

        req->tstate = CRLF;

    case CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }
}


static rstatus_t
_subrequest_store(struct request *req, struct mbuf *buf, bool cas)
{
    uint8_t *p;
    rstatus_t status;
    uint64_t num;
    bool end;

    enum token_store {
        KEY = 0,
        FLAG,
        EXPIRE,
        VLEN,
        NOREPLY,
        CRLF,
        SENTINEL
    } tstate;

    tstate = (enum token_store)req->tstate;
    ASSERT(tstate >= KEY && tstate < SENTINEL);

    switch (tstate) {
    case KEY:
        end = false;
        status = _chase_string(req, buf, &end, &_check_key);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = FLAG;

    case FLAG: /* fall-through intended */
        end = false;
        num = 0;
        status = _chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->flag = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = EXPIRE;

    case EXPIRE: /* fall-through intended */
        end = false;
        num = 0;
        status = _chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->flag = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = VLEN;

    case VLEN: /* fall-through intended */
        if (cas) {
            end = false;
        } else {
            end = true;
        }
        num = 0;
        status = _chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->vlen = (uint32_t)num;
        }
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = cas ? CAS : NOREPLY;

    case CAS: /* fall-through intended */
        if (cas) {
            end = true;
            num = 0;
            status = _chase_uint(&num, req, buf, &end, UINT64_MAX);
            if (status== CC_OK) {
                req->cas = num;
            }
            if (status != CC_OK || end) {
                return status;
            }

            req->tstate = NOREPLY;
        }

    case NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, &_check_noreply);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = CRLF;

    case CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }
}


static rstatus_t
_subrequest_retrieve(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    bool end;

    while (true) {
        end = true;
        status = _chase_string(req, buf, &end, &_check_key);
        if (status != CC_OK || end) {
            return status;
        }
    }
}

void
request_reset(struct request *req)
{
    req->rstate = PARSING;
    req->pstate = VERB;
    req->verb = UNKNOWN;

    req->keys->nelem = 0;
    req->flag = 0;
    req->expiry = 0;
    req->vlen = 0;
    req->delta = 0;
    req->cas = 0;

    req->noreply = 0;
    req->serror = 0;
    req->cerror = 0;
    req->swallow = 0;
}

rstatus_t request_init(struct request *req)
{
    rstatus_t status;

    ASSERT(req != NULL);

    status = array_alloc(&req->keys, MAX_BATCH_SIZE, sizeof(struct token));
    if (status != CC_OK) {
        return status;
    }

    request_reset(req);
    return CC_OK;
}

/* parse the first line("header") according to memcache ASCII protocol */
rstatus_t
request_parse_hdr(struct request *req, struct mbuf *buf)
{
    rstatus_t status;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);
    ASSERT(req->rstate == PARSING);

    if (req->pstate == VERB) {
        bool end = true;

        status = _chase_string(req, buf, &end, &_check_verb);
        if (status == CC_OK) {
            req->pstate = POST_VERB;
        } else {
            return status;
        }
    }

    if (req->pstate == POST_VERB) {
        switch (req->verb) {
        case GET:
        case GETS:
            status = _subrequest_retrieve(req, buf);
            break;

        case ADD:
        case SET:
        case REPLACE:
        case APPEND:
        case PREPEND:
        case CAS:
            status = _subrequest_store(req, buf);

            break;

        case INCR:
        case DECR:
            status = _subrequest_arithmetic(req, buf);

            break;

        case STATS:
        case QUIT:
            if (!*end) {
                req->swallow = 1;

                return CC_ERROR;
            }

            break;

        default:
            NOT_REACHED();
            break;
        }
    }

    return status;
}
