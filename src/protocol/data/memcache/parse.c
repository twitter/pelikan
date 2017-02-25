#include "parse.h"

#include "request.h"
#include "response.h"
#include "time/time.h"

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_debug.h>
#include <cc_define.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>

#define PARSE_MODULE_NAME "protocol::memcache::parse"

static bool parse_init = false;
static parse_req_metrics_st *parse_req_metrics = NULL;
static parse_rsp_metrics_st *parse_rsp_metrics = NULL;

void
parse_setup(parse_req_metrics_st *req, parse_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", PARSE_MODULE_NAME);

    if (parse_init) {
        log_warn("%s has already been setup, overwrite", PARSE_MODULE_NAME);
    }

    parse_req_metrics = req;
    parse_rsp_metrics = rsp;

    parse_init = true;
}

void
parse_teardown(void)
{
    log_info("tear down the %s module", PARSE_MODULE_NAME);

    if (!parse_init) {
        log_warn("%s has never been setup", PARSE_MODULE_NAME);
    }
    parse_req_metrics = NULL;
    parse_rsp_metrics = NULL;
    parse_init = false;
}

/*
 * common functions
 */
/* CRLF is special and we need to "peek into the future" */
static inline parse_rstatus_t
_try_crlf(struct buf *buf, char *p)
{
    if (*p != CR) {
        return PARSE_EINVALID;
    }

    if (buf->wpos == p + 1) { /* the next byte hasn't been received */
        return PARSE_EUNFIN;
    }

    if (*(p + 1) == LF) {
        return PARSE_OK;
    } else {
        return PARSE_EINVALID;
    }
}


static inline void
_token_begin(struct bstring *t, char *p)
{
    t->len = 1;
    t->data = p;
}

static inline bool
_token_end(bool *end, struct buf *buf, char *p)
{
    parse_rstatus_t status;

    status = _try_crlf(buf, p);
    if (status == PARSE_OK) {
        *end = true;
        return true;
    } else if (*p == ' ') {
        *end = false;
        return true;
    } else {
        *end = false;
        return false;
    }
}

static inline void
_forward_rpos(struct buf *buf, bool end, char *p)
{
    buf->rpos = end ? (p + CRLF_LEN) : (p + 1);
}

static inline bool
_token_oversize(struct buf *buf, char *p)
{
    /* TODO(yao): allow caller to provide token size limit for each field*/
    return (p - buf->rpos > MAX_TOKEN_LEN);
}


static parse_rstatus_t
_chase_crlf(struct buf *buf)
{
    char *p;
    parse_rstatus_t status;

    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        status = _try_crlf(buf, p);
        switch (status) {
        case PARSE_EUNFIN:
            return PARSE_EUNFIN;

        case PARSE_EINVALID: /* not CRLF */
            if (*p == ' ') {
                log_verb("unnecessary whitespace");
                break;
            } else {
                log_warn("ill formatted request: illegal character");
                return PARSE_EINVALID;
            }

        case PARSE_OK:
            buf->rpos = p;
            return PARSE_OK;

        default:
            NOT_REACHED();
            break;
        }
    }

    /* to get here, status has to be PARSE_EINVALID and the current character
     * has to be a whitespace. This indicates that there isn't enough data in
     * buf to fully parse the request, instead of an error.
     */
    return PARSE_EUNFIN;
}

static inline parse_rstatus_t
_check_key(struct buf *buf, bool *end, struct bstring *t, char *p)
{
    bool complete;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return PARSE_EUNFIN;
    }

    complete = _token_end(end, buf, p);
    if (complete) {
        _forward_rpos(buf, *end, p);

        if (t->len == 0) {
            return PARSE_EEMPTY;
        } else {
            return PARSE_OK;
        }
    }

    /* the current character is part of the key */
    if (t->len == 0) {
        _token_begin(t, p);
    } else {
        t->len++;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_chase_key(struct buf *buf, bool *end, struct bstring *t)
{
    char *p;
    parse_rstatus_t status;

    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        status = _check_key(buf, end, t, p);
        if (status != PARSE_EUNFIN) {
            return status;
        }
    }

    return PARSE_EUNFIN;
}

static inline parse_rstatus_t
_check_uint(uint64_t *num, struct buf *buf, bool *end, size_t *len,
        char *p, uint64_t max)
{
    bool complete;

    if (*p == ' ' && *len == 0) { /* pre-key spaces */
        return PARSE_EUNFIN;
    }

    complete = _token_end(end, buf, p);
    if (complete) {
        _forward_rpos(buf, *end, p);

        if (*len == 0) {
            log_warn("ill formatted request: no integer provided");

            return PARSE_EEMPTY;
        }

        /* we've already parsed every digit as we move through the token,
         * nothing else to do here
         */
        return PARSE_OK;
    }

    /* incomplete token, parse the current digit */
    if (isdigit(*p)) {
        if (*num > max / 10) {
            /* TODO(yao): catch the few numbers that will still overflow */
            log_warn("ill formatted request: integer too big");

            return PARSE_EINVALID;
        }

        (*len)++;
        *num = *num * 10ULL;
        *num += (uint64_t)(*p - '0');

        return PARSE_EUNFIN;
    } else {
        log_warn("ill formatted request: non-digit char in integer field");

        return PARSE_EINVALID;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_chase_uint(uint64_t *num, struct buf *buf, bool *end, uint64_t max)
{
    char *p;
    parse_rstatus_t status;
    size_t len = 0;

    *num = 0;
    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        status = _check_uint(num, buf, end, &len, p, max);
        if (status != PARSE_EUNFIN) {
            return status;
        }
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_parse_val(struct bstring *val, struct buf *buf, uint32_t nbyte)
{
    parse_rstatus_t status;
    uint32_t rsize = buf_rsize(buf);

    log_verb("parsing val (string) at %p", buf->rpos);

    val->len = MIN(nbyte, rsize);
    val->data = (val->len > 0) ? buf->rpos : NULL;
    buf->rpos += val->len;

    /* verify CRLF */
    if (rsize < nbyte + CRLF_LEN) {
        status = PARSE_EUNFIN;
    } else {
        status = _try_crlf(buf, buf->rpos);
        if (status == PARSE_OK) {
            buf->rpos += CRLF_LEN;
        } else {
            log_debug("CRLF expected at %p, '%c%c' found instead", buf->rpos,
                    *buf->rpos, *(buf->rpos + 1));
        }
    }

    log_verb("buf %p has %"PRIu32" out of the %"PRIu32" bytes expected", buf,
            rsize, nbyte + CRLF_LEN);

    return status;
}

/*
 * request specific functions
 */

static inline parse_rstatus_t
_check_req_type(struct request *req, struct buf *buf, bool *end, struct bstring *t,
        char *p)
{
    bool complete;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return PARSE_EUNFIN;
    }

    complete = _token_end(end, buf, p);
    if (complete) {
        _forward_rpos(buf, *end, p);

        if (t->len == 0) {
            log_warn("ill formatted request: empty request");

            return PARSE_EEMPTY;
        }

        switch (t->len) {
        case 3:
            if (str3cmp(t->data, 'g', 'e', 't')) {
                req->type = REQ_GET;
                break;
            }

            if (str3cmp(t->data, 's', 'e', 't')) {
                req->type = REQ_SET;
                break;
            }

            if (str3cmp(t->data, 'a', 'd', 'd')) {
                req->type = REQ_ADD;
                break;
            }

            if (str3cmp(t->data, 'c', 'a', 's')) {
                req->type = REQ_CAS;
                break;
            }

            break;

        case 4:
            if (str4cmp(t->data, 'g', 'e', 't', 's')) {
                req->type = REQ_GETS;
                break;
            }

            if (str4cmp(t->data, 'i', 'n', 'c', 'r')) {
                req->type = REQ_INCR;
                break;
            }

            if (str4cmp(t->data, 'd', 'e', 'c', 'r')) {
                req->type = REQ_DECR;
                break;
            }

            if (str4cmp(t->data, 'q', 'u', 'i', 't')) {
                req->type = REQ_QUIT;
                break;
            }

            break;

        case 6:
            if (str6cmp(t->data, 'd', 'e', 'l', 'e', 't', 'e')) {
                req->type = REQ_DELETE;
                break;
            }

            if (str6cmp(t->data, 'a', 'p', 'p', 'e', 'n', 'd')) {
                req->type = REQ_APPEND;
                break;
            }

            break;

        case 7:
            if (str7cmp(t->data, 'r', 'e', 'p', 'l', 'a', 'c', 'e')) {
                req->type = REQ_REPLACE;
                break;
            }

            if (str7cmp(t->data, 'p', 'r', 'e', 'p', 'e', 'n', 'd')) {
                req->type = REQ_PREPEND;
                break;
            }

            break;

        case 9:
            if (str9cmp(t->data, 'f', 'l', 'u', 's', 'h', '_', 'a', 'l', 'l')) {
                req->type = REQ_FLUSH;
                break;
            }

            break;
        }

        if (req->type == REQ_UNKNOWN) { /* no match */
            log_warn("ill formatted request: unknown command");

            return PARSE_EINVALID;
        } else {
            _forward_rpos(buf, *end, p);

            return PARSE_OK;
        }
    }

    /* token incomplete */
    if (t->len == 0) {
        _token_begin(t, p);
    } else {
        t->len++;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_chase_req_type(struct request *req, struct buf *buf, bool *end)
{
    char *p;
    parse_rstatus_t status;
    struct bstring t;

    bstring_init(&t);
    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        status = _check_req_type(req, buf, end, &t, p);
        if (status != PARSE_EUNFIN) {
            return status;
        }
    }

    return PARSE_EUNFIN;
}

static inline parse_rstatus_t
_push_key(struct request *req, struct bstring *t)
{
    struct bstring *k;

    if (array_nelem(req->keys) >= MAX_BATCH_SIZE) {
          log_warn("ill formatted request: too many keys in a batch");

          return PARSE_EOTHER;
      }

      /* push should never fail as keys are preallocated for MAX_BATCH_SIZE */
      k = array_push(req->keys);
      *k = *t;

      return PARSE_OK;
}


static inline parse_rstatus_t
_check_noreply(struct buf *buf, bool *end, struct bstring *t, char *p)
{
    bool complete;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return PARSE_EUNFIN;
    }

    complete = _token_end(end, buf, p);
    if (complete) {
        _forward_rpos(buf, *end, p);

        if (t->len == 0) {
            return PARSE_EEMPTY;
        }

        if (t->len == 7 && str7cmp(t->data, 'n', 'o', 'r', 'e', 'p', 'l', 'y')) {
            return PARSE_OK;
        }

        return PARSE_EINVALID;
    }

    /* token not complete */
    if (t->len == 0) {
        _token_begin(t, p);
    } else {
        t->len++;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_chase_noreply(struct request *req, struct buf *buf, bool *end)
{
    char *p;
    struct bstring t;

    bstring_init(&t);
    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        switch (_check_noreply(buf, end, &t, p)) {
        case PARSE_EUNFIN:
            break;
        case PARSE_OK:
            req->noreply = 1;
        /* fall-through intended */
        case PARSE_EEMPTY: /* noreply is optional, empty token OK */
            return PARSE_OK;
            break;

        default:
            return PARSE_EINVALID;
            break;
        }
    }

    /* if we get here, there can only be one state */
    return PARSE_EUNFIN;
}


static parse_rstatus_t
_subrequest_delete(struct request *req, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    struct bstring t;

    /* parsing order:
     *   KEY
     *   NOREPLY, optional
     */

    bstring_init(&t);
    /* KEY */
    status = _chase_key(buf, end, &t);
    if (status == PARSE_OK) {
        status = _push_key(req, &t);
    }
    if (status != PARSE_OK || *end) {
        return status;
    }
    /* NOREPLY, optional */
    return _chase_noreply(req, buf, end);
}

static parse_rstatus_t
_subrequest_arithmetic(struct request *req, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    uint64_t delta;
    struct bstring t;

    /* parsing order:
     *   KEY
     *   DELTA,
     *   NOREPLY, optional
     */

    bstring_init(&t);
    /* KEY */
    status = _chase_key(buf, end, &t);
    if (status == PARSE_OK) {
        status = _push_key(req, &t);
    }
    if (status != PARSE_OK) {
        return status;
    }
    /* DELTA */
    if (*end) {
        goto incomplete;
    }
    delta = 0;
    status = _chase_uint(&delta, buf, end, UINT64_MAX);
    if (status == PARSE_OK) {
        req->delta = delta;
    }
    if (status != PARSE_OK || *end) {
        return status;
    }
    /* NOREPLY, optional */
    return _chase_noreply(req, buf, end);

incomplete:
    log_warn("ill formatted request: missing field(s) in arithmetic cmd");

    return PARSE_EOTHER;
}


static parse_rstatus_t
_subrequest_store(struct request *req, struct buf *buf, bool *end, bool cas)
{
    parse_rstatus_t status;
    uint64_t n;
    struct bstring t;

    /* parsing order:
     *   KEY
     *   FLAG
     *   EXPIRE
     *   VLEN
     *   CAS (conditional)
     *   NOREPLY, optional
     */

    bstring_init(&t);
    /* KEY */
    status = _chase_key(buf, end, &t);
    if (status == PARSE_OK) {
        status = _push_key(req, &t);
    }
    if (status != PARSE_OK) {
        return status;
    }
    /* FLAG */
    if (*end) {
        goto incomplete;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT32_MAX);
    if (status != PARSE_OK) {
        return status;
    }
    req->flag = (uint32_t)n;
    /* EXPIRE */
    if (*end) {
        goto incomplete;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT32_MAX);
    if (status != PARSE_OK) {
        return status;
    }
    req->expiry = (uint32_t)n;
    /* VLEN */
    if (*end) {
        goto incomplete;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT32_MAX);
    if (status != PARSE_OK) {
        return status;
    }
    req->vlen = (uint32_t)n;
    req->nremain = req->vlen;
    /* CAS, conditional */
    if (cas) {
        if (*end) {
            goto incomplete;
        }
        n = 0;
        status = _chase_uint(&n, buf, end, UINT64_MAX);
        req->vcas = n;
        if (status != PARSE_OK || *end) {
            return status;
        }
    }
    /* NOREPLY, optional */
    if (*end) {
        return PARSE_OK;
    }
    return _chase_noreply(req, buf, end);

incomplete:
    log_warn("ill formatted request: missing field(s) in store command");

    return PARSE_EOTHER;
}


static parse_rstatus_t
_subrequest_retrieve(struct request *req, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    struct bstring t;

    while (true) {
        bstring_init(&t);
        status = _chase_key(buf, end, &t);
        if (status == PARSE_OK) {
            status = _push_key(req, &t);
        } else if (status == PARSE_EEMPTY) {
            ASSERT(*end);

            if (array_nelem(req->keys) == 0) {
                log_warn("ill formatted request: missing field(s) in retrieve "
                        "command");

                return PARSE_EOTHER;
            } else {
                return PARSE_OK;
            }
        }
        if (status != PARSE_OK || *end) {
            return status;
        }
    }
}

/* parse the first line("header") according to memcache ASCII protocol */
static parse_rstatus_t
_parse_req_hdr(struct request *req, struct buf *buf)
{
    parse_rstatus_t status;
    bool end = false;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);

    log_verb("parsing hdr at %p into req %p", buf->rpos, req);

    /* get the verb first */
    status = _chase_req_type(req, buf, &end);
    if (status != PARSE_OK) {
        return status;
    }

    /* rest of the request header */
    switch (req->type) {
    case REQ_GET:
    case REQ_GETS:
        status = _subrequest_retrieve(req, buf, &end);
        break;

    case REQ_DELETE:
        status = _subrequest_delete(req, buf, &end);
        break;

    case REQ_ADD:
    case REQ_SET:
    case REQ_REPLACE:
    case REQ_APPEND:
    case REQ_PREPEND:
        req->val = 1;
        status = _subrequest_store(req, buf, &end, false);
        break;

    case REQ_CAS:
        req->val = 1;
        status = _subrequest_store(req, buf, &end, true);
        break;

    case REQ_INCR:
    case REQ_DECR:
        status = _subrequest_arithmetic(req, buf, &end);
        break;

    /* flush_all can take a delay e.g. 'flush_all 10\r\n', not implemented */
    case REQ_FLUSH:
    case REQ_QUIT:
        break;

    default:
        NOT_REACHED();
        return PARSE_EOTHER;
    }

    if (status != PARSE_OK) {
        return status;
    }
    if (!end) {
        status = _chase_crlf(buf);
    }

    return status;
}

parse_rstatus_t
parse_req(struct request *req, struct buf *buf)
{
    parse_rstatus_t status = PARSE_OK;
    char *old_rpos = buf->rpos;
    bool leftmost = (buf->rpos == buf->begin);

    /*
     * we allow partial value in the request (but not the head portion),
     * so that we can incrementally fill in a large value over multiple socket
     * reads. This is more useful for the server which allows more predictable
     * buffer management (e.g. no unbounded read buffer). Currently partial
     * value is not implemented for the response.
     */
    switch (req->rstate) {
    case REQ_PARSING: /* a new request */
        log_verb("parsing buf %p into req %p", buf, req);
        req->first = true;
        status = _parse_req_hdr(req, buf);
        if (status == PARSE_EUNFIN) {
            log_verb("incomplete data: reset read position, jump back %zu bytes",
                    buf->rpos - old_rpos);
            /* return and start from beginning next time */
            request_reset(req);
            buf->rpos = old_rpos;
            break;
        }
        log_verb("request hdr parsed: %zu bytes scanned, parsing status %d",
                buf->rpos - old_rpos, status);
        if (req->val == 0 || status != PARSE_OK) {
            req->rstate = REQ_PARSED;
            break;
        }
        /* fall-through intended */

    case REQ_PARTIAL: /* continuation of value parsing for the current request */
        status = _parse_val(&(req->vstr), buf, req->nremain);
        req->nremain -= req->vstr.len;
        log_verb("this value segment: %"PRIu32", remain: %"PRIu32, req->vstr.len,
                req->nremain);
        if (status == PARSE_OK) {
            req->rstate = REQ_PARSED;
            req->partial = false;
            INCR(parse_req_metrics, request_parse);
        } else {
            if (status != PARSE_EUNFIN) {
                log_debug("parse req returned error state %d", status);
                req->cerror = 1;
                INCR(parse_req_metrics, request_parse_ex);
            } else { /* partial val, we return upon partial header above */
                /*
                 * We try to fit as much data into read buffer as possible
                 * before processing starts. When request starts somewhere in
                 * the middle of buf, we jump back and wait for more data to
                 * arrive (and expect caller to left-shift data in buf)
                 *
                 * This is a seemingly weird and unnecessary decision, the
                 * reason we need to do this is because we want to allow
                 * partial value only for set/add/cas/replace, but not for
                 * append/prepend. Because append/prepend are modifying keys
                 * already linked into hash, if we want to support partial
                 * value we need to either copy the key/value or temporarily
                 * unlink the key. And either option has severe drawbacks. Given
                 * append/prepend very large value is a case that I've never
                 * seen any need for in the field, it's a reasonable assumption
                 * to make, at least for now.
                 *
                 * With this behavior in place, the processing logic can assume
                 * that if it sees a partial request for append/prepend, the
                 * payload is too big to be held in the read buffer, without the
                 * possibility that a small append request just happens to come
                 * behind a number of other requests.
                 */

                if (leftmost) {
                    req->partial = true;
                    status = PARSE_OK;
                    req->rstate = REQ_PARTIAL;
                } else {
                    ASSERT(req->first == 1);
                    log_verb("try to left shift a request when possible");
                    request_reset(req);
                    buf->rpos = old_rpos;
                }
            }
        }
        break;

    default:
        NOT_REACHED();
        status = PARSE_EOTHER;
    }

    return status;
}


/*
 * response specific functions
 */

static inline parse_rstatus_t
_check_rsp_type(struct response *rsp, struct buf *buf, bool *end, struct bstring *t,
        char *p)
{
    bool complete;

    if (*p == ' ' && t->len == 0) { /* pre-key spaces */
        return PARSE_EUNFIN;
    }

    complete = _token_end(end, buf, p);
    if (complete) {
        _forward_rpos(buf, *end, p);

        if (t->len == 0) {
            log_warn("ill formatted response: empty response");

            return PARSE_EEMPTY;
        }

        switch (t->len) {
        case 2:
            if (str2cmp(t->data, 'O', 'K')) {
                rsp->type = RSP_OK;
                break;
            }
            break;

        case 3:
            if (str3cmp(t->data, 'E', 'N', 'D')) {
                rsp->type = RSP_END;
                break;
            }
            break;

        case 4:
            if (str4cmp(t->data, 'S', 'T', 'A', 'T')) {
                rsp->type = RSP_STAT;
                break;
            }
            break;

        case 5:
            if (str5cmp(t->data, 'V', 'A', 'L', 'U', 'E')) {
                rsp->type = RSP_VALUE;
                break;
            }
            break;

        case 6:
            if (str6cmp(t->data, 'E', 'X', 'I', 'S', 'T', 'S')) {
                rsp->type = RSP_EXISTS;
                break;
            }
            if (str6cmp(t->data, 'S', 'T', 'O', 'R', 'E', 'D')) {
                rsp->type = RSP_STORED;
                break;
            }
            break;

        case 7:
            if (str7cmp(t->data, 'D', 'E', 'L', 'E', 'T', 'E', 'D')) {
                rsp->type = RSP_DELETED;
                break;
            }
            break;

        case 9:
            if (str9cmp(t->data, 'N', 'O', 'T', '_', 'F', 'O', 'U', 'N', 'D')) {
                rsp->type = RSP_NOT_FOUND;
                break;
            }
            break;

        case 10:
            if (str10cmp(t->data, 'N', 'O', 'T', '_', 'S', 'T', 'O', 'R', 'E',
                        'D')) {
                rsp->type = RSP_NOT_STORED;
                break;
            }
            break;

        case 12:
            if (str12cmp(t->data, 'C', 'L', 'I', 'E', 'N', 'T', '_', 'E', 'R',
                        'R', 'O', 'R')) {
                rsp->type = RSP_CLIENT_ERROR;
                break;
            }
            if (str12cmp(t->data, 'S', 'E', 'R', 'V', 'E', 'R', '_', 'E', 'R',
                        'R', 'O', 'R')) {
                rsp->type = RSP_SERVER_ERROR;
                break;
            }
            break;
        default:
            break;
        }

        if (rsp->type == RSP_UNKNOWN) { /* no match */
            log_warn("ill formatted request: unknown command");

            return PARSE_EINVALID;
        } else {
            _forward_rpos(buf, *end, p);

            return PARSE_OK;
        }
    }

    /* token incomplete */
    if (t->len == 0) {
        _token_begin(t, p);
    } else {
        t->len++;
    }

    return PARSE_EUNFIN;
}


static parse_rstatus_t
_chase_rsp_type(struct response *rsp, struct buf *buf, bool *end)
{
    char *p;
    parse_rstatus_t status;
    uint64_t n = 0;
    struct bstring t;

    bstring_init(&t);
    p = buf->rpos;

    if (isdigit(*p)) { /* response is likely a numeric value for incr/decr */
        rsp->type = RSP_NUMERIC;
        status = _chase_uint(&n, buf, end, UINT64_MAX);
        if (status == PARSE_OK) {
            rsp->num = 1;
            rsp->vint = n;
        }

        return status;
    }

    /* not a number, must be one of the types in response_type_t */
    for (; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        status = _check_rsp_type(rsp, buf, end, &t, p);
        if (status != PARSE_EUNFIN) {
            return status;
        }
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_subresponse_stat(struct response *rsp, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    uint64_t n;
    struct bstring t;

    /* parsing order:
     *   T_KEY
     *   T_NUM
     */

    bstring_init(&t);
    /* KEY */
    status = _chase_key(buf, end, &t);
    if (status != PARSE_OK) {
        return status;
    }
    rsp->key = t;
    /* NUM */
    if (*end) {
        log_warn("ill formatted request: missing field(s) in stats response");

        return PARSE_EOTHER;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT64_MAX);
    rsp->num = 1;
    rsp->vint = n;

    return status;
}

static parse_rstatus_t
_subresponse_value(struct response *rsp, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    uint64_t n;
    struct bstring t;

    /* parsing order:
     *   T_KEY
     *   T_FLAG
     *   T_VLEN
     *   T_CAS, optional/conditional
     */

    bstring_init(&t);
    /* KEY */
    status = _chase_key(buf, end, &t);
    if (status != PARSE_OK) {
        return status;
    }
    rsp->key = t;
    /* FLAG */
    if (*end) {
        goto incomplete;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT32_MAX);
    if (status != PARSE_OK) {
        return status;
    }
    rsp->flag = (uint32_t)n;
    /* VLEN */
    if (*end) {
        goto incomplete;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT32_MAX);
    if (status != PARSE_OK) {
        return status;
    }
    rsp->vlen = (uint32_t)n;
    /* CAS */
    if (*end) {
        return PARSE_OK;
    }
    n = 0;
    status = _chase_uint(&n, buf, end, UINT64_MAX);
    rsp->vcas = n;
    return status;

incomplete:
    log_warn("ill formatted response: missing field(s) in value response");

    return PARSE_EOTHER;
}

static parse_rstatus_t
_subresponse_error(struct response *rsp, struct buf *buf, bool *end)
{
    parse_rstatus_t status;
    char *p;
    struct bstring t;

    bstring_init(&t);
    for (p = buf->rpos; p < buf->wpos; p++) {
        if (_token_oversize(buf, p)) {
            return PARSE_EOVERSIZE;
        }

        if (*p == ' ' && t.len == 0) { /* pre-message spaces */
            break;
        }

        status = _try_crlf(buf, p);
        switch (status) {
        case PARSE_EUNFIN:
            break;

        case PARSE_EINVALID:
            if (t.len == 0) {
                _token_begin(&t, p);
            } else {
                t.len++;
            }
            break;

        case PARSE_OK:
            rsp->vstr = t;
            *end = true;
            _forward_rpos(buf, *end, p);

            return PARSE_OK;

        default:
            NOT_REACHED();
            return PARSE_EOTHER;
        }
    }

    return PARSE_EUNFIN;
}


static parse_rstatus_t
_parse_rsp_hdr(struct response *rsp, struct buf *buf)
{
    parse_rstatus_t status;
    bool end = false;

    ASSERT(rsp != NULL);
    ASSERT(buf != NULL);

    log_verb("parsing hdr at %p into rsp %p", buf->rpos, rsp);

    /* get the type first */
    status = _chase_rsp_type(rsp, buf, &end);
    if (status != PARSE_OK) {
        return status;
    }

    /* rest of the response (first line) */
    switch (rsp->type) {
    case RSP_STAT:
        status = _subresponse_stat(rsp, buf, &end);
        break;

    case RSP_VALUE:
        rsp->val = 1;
        status = _subresponse_value(rsp, buf, &end);
        break;

    case RSP_CLIENT_ERROR:
    case RSP_SERVER_ERROR:
        if (!end) {
            status = _subresponse_error(rsp, buf, &end);
        }
        break;

    case RSP_OK:
    case RSP_END:
    case RSP_EXISTS:
    case RSP_STORED:
    case RSP_DELETED:
    case RSP_NOT_FOUND:
    case RSP_NOT_STORED:
    case RSP_NUMERIC:
        if (!end) {
            return PARSE_EINVALID;
        }
        break;

    default:
        NOT_REACHED();
        return PARSE_EOTHER;
    }

    if (status != PARSE_OK) {
        return status;
    }
    if (!end) {
        status = _chase_crlf(buf);
    }

    return status;
}

parse_rstatus_t
parse_rsp(struct response *rsp, struct buf *buf)
{
    parse_rstatus_t status = PARSE_EUNFIN;
    char *old_rpos = buf->rpos;

    ASSERT(rsp->rstate == RSP_PARSING);

    log_verb("parsing buf %p into rsp %p", buf, rsp);

    status = _parse_rsp_hdr(rsp, buf);
    if (status == CC_OK && rsp->val) {
        status = _parse_val(&rsp->vstr, buf, rsp->vlen);
    }

    if (status == PARSE_EUNFIN) {
        log_verb("incomplete data: reset read position, jump back %zu bytes",
                buf->rpos - old_rpos);
        buf->rpos = old_rpos; /* start from beginning next time */
        return PARSE_EUNFIN;
    }
    if (status != PARSE_OK) {
        log_debug("parse rsp returned error state %d", status);
        rsp->error = 1;
        INCR(parse_rsp_metrics, response_parse_ex);
    } else {
        rsp->rstate = RSP_PARSED;
        INCR(parse_rsp_metrics, response_parse);
    }

    return status;
}
