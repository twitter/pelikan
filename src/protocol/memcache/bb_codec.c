#include <protocol/memcache/bb_codec.h>

#include <time/bb_time.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_debug.h>
#include <cc_define.h>
#include <cc_log.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>

#define CODEC_MODULE_NAME "protocol::memcache::codec"

static bool codec_init = false;
static codec_metrics_st *codec_metrics = NULL;

void
codec_setup(codec_metrics_st *metrics)
{
    log_info("set up the %s module", CODEC_MODULE_NAME);

    codec_metrics = metrics;
    CODEC_METRIC_INIT(codec_metrics);

    if (codec_init) {
        log_warn("%s has already been setup, overwrite", CODEC_MODULE_NAME);
    }
    codec_init = true;
}

void
codec_teardown(void)
{
    log_info("tear down the %s module", CODEC_MODULE_NAME);

    if (!codec_init) {
        log_warn("%s has never been setup", CODEC_MODULE_NAME);
    }
    codec_metrics = NULL;
    codec_init = false;
}

/* functions related to parsing messages */

typedef rstatus_t (*check_token_t)(struct request *req, struct buf *buf,
        bool *end, struct bstring *t, uint8_t *p);

static inline void
_mark_cerror(struct request *req, struct buf *buf, uint8_t *npos)
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

    INCR(codec_metrics, request_parse_ex);
}

static inline void
_token_start(struct bstring *t, uint8_t *p)
{
    t->len = 1;
    t->data = p;
}

/*
 * NOTE(yao): In the following parser/subparser functions, we move the rpos
 * pointer in buf forward when we finish parsing a token fully. This simplifies
 * the state machine.
 */

static inline rstatus_t
_token_check_size(struct request *req, struct buf *buf, uint8_t *p)
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
_try_crlf(struct buf *buf, uint8_t *p)
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
_chase_crlf(struct request *req, struct buf *buf)
{
    uint8_t *p;
    rstatus_t status;

    for (p = buf->rpos; p < buf->wpos; p++) {
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
                log_verb("unnecessary whitespace");
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
_check_key(struct request *req, struct buf *buf, bool *end,
        struct bstring *t, uint8_t *p)
{
    rstatus_t status;
    struct bstring *k; /* a key token */
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
                    buf->rpos = p + CRLF_LEN;
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

        k->data = t->data;
        k->len = t->len;
        buf->rpos = *end ? (p + CRLF_LEN) : (p + 1);

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
_check_verb(struct request *req, struct buf *buf, bool *end, struct bstring *t, uint8_t *p)
{
    rstatus_t status;
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
                log_warn("ill formatted request: empty request");

                goto error;
            }

            complete = true;
        }
    }

    if (complete) {
        ASSERT(req->verb == REQ_UNKNOWN);

        switch (p - t->data) {
        case 3:
            if (str3cmp(t->data, 'g', 'e', 't')) {
                req->verb = REQ_GET;
                break;
            }

            if (str3cmp(t->data, 's', 'e', 't')) {
                req->verb = REQ_SET;
                break;
            }

            if (str3cmp(t->data, 'a', 'd', 'd')) {
                req->verb = REQ_ADD;
                break;
            }

            if (str3cmp(t->data, 'c', 'a', 's')) {
                req->verb = REQ_CAS;
                break;
            }

            break;

        case 4:
            if (str4cmp(t->data, 'g', 'e', 't', 's')) {
                req->verb = REQ_GETS;
                break;
            }

            if (str4cmp(t->data, 'i', 'n', 'c', 'r')) {
                req->verb = REQ_INCR;
                break;
            }

            if (str4cmp(t->data, 'd', 'e', 'c', 'r')) {
                req->verb = REQ_DECR;
                break;
            }

            if (str4cmp(t->data, 'q', 'u', 'i', 't')) {
                req->verb = REQ_QUIT;
                break;
            }

            break;

        case 5:
            if (str5cmp(t->data, 's', 't', 'a', 't', 's')) {
                req->verb = REQ_STATS;
                break;
            }

            break;

        case 6:
            if (str6cmp(t->data, 'd', 'e', 'l', 'e', 't', 'e')) {
                req->verb = REQ_DELETE;
                break;
            }

            if (str6cmp(t->data, 'a', 'p', 'p', 'e', 'n', 'd')) {
                req->verb = REQ_APPEND;
                break;
            }

            break;

        case 7:
            if (str7cmp(t->data, 'r', 'e', 'p', 'l', 'a', 'c', 'e')) {
                req->verb = REQ_REPLACE;
                break;
            }

            if (str7cmp(t->data, 'p', 'r', 'e', 'p', 'e', 'n', 'd')) {
                req->verb = REQ_PREPEND;
                break;
            }

            break;
        }

        if (req->verb == REQ_UNKNOWN) { /* no match */
            _mark_cerror(req, buf, p);

            return CC_ERROR;
        } else {
            buf->rpos = *end ? (p + CRLF_LEN) : (p + 1);

            return CC_OK;
        }
    }


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
_check_noreply(struct request *req, struct buf *buf, bool *end,
        struct bstring *t, uint8_t *p)
{
    rstatus_t status;
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
        if (t->len == 7 && str7cmp(t->data, 'n', 'o', 'r', 'e', 'p', 'l', 'y')) {
            req->noreply = 1;
            buf->rpos = *end ? (p + CRLF_LEN) : (p + 1);

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
_chase_string(struct request *req, struct buf *buf, bool *end,
        check_token_t checker)
{
    uint8_t *p;
    rstatus_t status;
    struct bstring t;

    bstring_init(&t);
    for (p = buf->rpos; p < buf->wpos; p++) {
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
_check_uint(uint64_t *num, struct request *req, struct buf *buf, bool *end,
        struct bstring *t, uint8_t *p, uint64_t max)
{
    rstatus_t status;
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
        log_vverb("end?: %d, num: %"PRIu64, *end, num);

        buf->rpos = *end ? (p + CRLF_LEN) : (p + 1);
        return CC_OK;
    }

    if (isdigit(*p)) {
        if (*num > max / 10) {
            /* TODO(yao): catch the few numbers that will still overflow */

            log_warn("ill formatted request: integer too big");

            goto error;
        }

        t->len++;
        *num = *num * 10ULL;
        *num += (uint64_t)(*p - '0');

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
_chase_uint(uint64_t *num, struct request *req, struct buf *buf, bool *end,
        uint64_t max)
{
    uint8_t *p;
    rstatus_t status;
    struct bstring t;

    *num = 0;
    bstring_init(&t);
    for (p = buf->rpos; p < buf->wpos; p++) {
        status = _token_check_size(req, buf, p);
        if (status != CC_OK) {
            return CC_ERROR;
        }

        status = _check_uint(num, req, buf, end, &t, p, max);
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
_subrequest_delete(struct request *req, struct buf *buf)
{
    rstatus_t status;
    bool end;

    enum token_delete {
        T_KEY = 0,
        T_NOREPLY,
        T_CRLF,
        T_SENTINEL
    } tstate;

    tstate = (enum token_delete)req->tstate;
    ASSERT(tstate >= T_KEY && tstate < T_SENTINEL);

    switch (tstate) {
    case T_KEY:
        end = true;
        status = _chase_string(req, buf, &end, _check_key);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = T_NOREPLY;

    case T_NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, _check_noreply);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = T_CRLF;

    case T_CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }

    NOT_REACHED();
    return CC_ERROR;
}


static rstatus_t
_subrequest_arithmetic(struct request *req, struct buf *buf)
{
    rstatus_t status;
    uint64_t delta;
    bool end;

    enum token_arithmetic {
        T_KEY = 0,
        T_DELTA,
        T_NOREPLY,
        T_CRLF,
        T_SENTINEL
    } tstate;

    tstate = (enum token_arithmetic)req->tstate;
    ASSERT(tstate >= T_KEY && tstate < T_SENTINEL);

    switch (tstate) {
    case T_KEY:
        end = false;
        status = _chase_string(req, buf, &end, _check_key);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = T_DELTA;

    case T_DELTA: /* fall-through intended */
        end = true;
        delta = 0;
        status = _chase_uint(&delta, req, buf, &end, UINT64_MAX);
        if (status== CC_OK) {
            req->delta = delta;
        }
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = T_NOREPLY;

    case T_NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, _check_noreply);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = T_CRLF;

    case T_CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }

    NOT_REACHED();
    return CC_ERROR;
}


static rstatus_t
_subrequest_store(struct request *req, struct buf *buf, bool cas)
{
    rstatus_t status;
    uint64_t num;
    bool end;

    enum token_store {
        T_KEY = 0,
        T_FLAG,
        T_EXPIRE,
        T_VLEN,
        T_CAS,
        T_NOREPLY,
        T_CRLF,
        T_SENTINEL
    } tstate;

    tstate = (enum token_store)req->tstate;
    ASSERT(tstate >= T_KEY && tstate < T_SENTINEL);

    switch (tstate) {
    case T_KEY:
        end = false;
        status = _chase_string(req, buf, &end, _check_key);
        if (status != CC_OK) {
            return status;
        }

        req->tstate = T_FLAG;

    case T_FLAG: /* fall-through intended */
        end = false;
        num = 0;
        status = _chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->flag = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = T_EXPIRE;

    case T_EXPIRE: /* fall-through intended */
        end = false;
        num = 0;
        status = _chase_uint(&num, req, buf, &end, UINT32_MAX);
        if (status== CC_OK) {
            req->expiry = (uint32_t)num;
        } else {
            return status;
        }

        req->tstate = T_VLEN;

    case T_VLEN: /* fall-through intended */
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

        req->tstate = cas ? T_CAS : T_NOREPLY;

    case T_CAS: /* fall-through intended */
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

            req->tstate = T_NOREPLY;
        }

    case T_NOREPLY: /* fall-through intended */
        end = true;
        status = _chase_string(req, buf, &end, _check_noreply);
        if (status != CC_OK || end) {
            return status;
        }

        req->tstate = T_CRLF;

    case T_CRLF: /* fall-through intended */
        return _chase_crlf(req, buf);

    default:
        NOT_REACHED();
        break;
    }

    NOT_REACHED();
    return CC_ERROR;
}


static rstatus_t
_subrequest_retrieve(struct request *req, struct buf *buf)
{
    rstatus_t status;
    bool end;

    while (true) {
        end = true;
        status = _chase_string(req, buf, &end, _check_key);
        if (status != CC_OK || end) {
            return status;
        }
    }

    NOT_REACHED();
    return CC_ERROR;
}

/* swallowing the current line, delimited by '\r\n' */
rstatus_t
parse_swallow(struct buf *buf)
{
    uint8_t *p;
    rstatus_t status;

    for (p = buf->rpos; p < buf->wpos; p++) {
        status = _try_crlf(buf, p);
        switch (status) {
        case CC_UNFIN:
            buf->rpos = p;

            return CC_UNFIN;

        case CC_ERROR: /* not CRLF */
            break;

        case CC_OK:
            log_verb("swallowed %zu bytes", p + CRLF_LEN - buf->rpos);
            INCR(codec_metrics, request_swallow);
            buf->rpos = p + CRLF_LEN;

            return CC_OK;

        default:
            NOT_REACHED();
            break;
        }
    }

    /* the line isn't finished yet */
    return CC_UNFIN;
}

/* parse the first line("header") according to memcache ASCII protocol */
rstatus_t
parse_req_hdr(struct request *req, struct buf *buf)
{
    rstatus_t status;
    uint8_t *rpos;
    bool end;

    ASSERT(req != NULL);
    ASSERT(buf != NULL);
    ASSERT(req->rstate == PARSING);
    ASSERT(req->pstate == REQ_HDR);

    log_verb("parsing hdr at %p into req %p", buf->rpos, req);

    rpos = buf->rpos;

    /* get the verb first */
    end = true;
    status = _chase_string(req, buf, &end, _check_verb);
    if (status != CC_OK) {
        return status;
    }

    /* rest of the request header */
    switch (req->verb) {
    case REQ_GET:
    case REQ_GETS:
        status = _subrequest_retrieve(req, buf);

        break;

    case REQ_DELETE:
        status = _subrequest_delete(req, buf);

        break;

    case REQ_ADD:
    case REQ_SET:
    case REQ_REPLACE:
    case REQ_APPEND:
    case REQ_PREPEND:
        req->pstate = REQ_VAL;
        status = _subrequest_store(req, buf, false);

        break;

    case REQ_CAS:
        req->pstate = REQ_VAL;
        status = _subrequest_store(req, buf, true);

        break;

    case REQ_INCR:
    case REQ_DECR:
        status = _subrequest_arithmetic(req, buf);

        break;

    case REQ_STATS:
    case REQ_QUIT:
        if (!end) {
            status = _chase_crlf(req, buf);
            if (status == CC_ERROR) {
                req->swallow = 1;
            }
        }

        break;

    default:
        NOT_REACHED();
        break;
    }

    if (status == CC_UNFIN) { /* reset rpos if the hdr is incomplete */
        buf->rpos = rpos;
    }

    return status;
}

rstatus_t
parse_req_val(struct request *req, struct buf *buf)
{
    rstatus_t status;

    log_verb("parsing val at %p into req %p", buf->rpos, req);

    if (buf_rsize(buf) < req->vlen + CRLF_LEN) {
        log_verb("rbuf has %"PRIu32" out of the %"PRIu32" bytes "
                "expected", buf_rsize(buf), req->vlen + CRLF_LEN);

        return CC_UNFIN;
    }

    req->vstr.len = req->vlen;
    req->vstr.data = buf->rpos;

    buf->rpos += req->vlen;

    /* verify CRLF */
    status = _try_crlf(buf, buf->rpos);
    if (status == CC_OK) {
        buf->rpos += CRLF_LEN;
    } else {
        _mark_cerror(req, buf, buf->rpos);
    }

    return status;
}

rstatus_t
parse_req(struct request *req, struct buf *buf)
{
    rstatus_t status;

    ASSERT(req->rstate == PARSING);

    log_verb("parsing buf %p into req %p (state: %d)", buf, req,
            req->pstate);

    if (req->pstate == REQ_HDR) {
        status = parse_req_hdr(req, buf);
        if (status != CC_OK) {
            goto done;
        }
    }

    if (req->pstate == REQ_VAL) {
        status = parse_req_val(req, buf);
    }

    if (status == CC_OK) {
        req->rstate = PARSED;
        INCR(codec_metrics, cmd_total);
        switch (req->verb) {
        case REQ_GET:
            INCR(codec_metrics, cmd_get);
            break;
        case REQ_GETS:
            INCR(codec_metrics, cmd_gets);
            break;
        case REQ_DELETE:
            INCR(codec_metrics, cmd_delete);
            break;
        case REQ_ADD:
            INCR(codec_metrics, cmd_add);
            break;
        case REQ_SET:
            INCR(codec_metrics, cmd_set);
            break;
        case REQ_REPLACE:
            INCR(codec_metrics, cmd_replace);
            break;
        case REQ_APPEND:
            INCR(codec_metrics, cmd_append);
            break;
        case REQ_PREPEND:
            INCR(codec_metrics, cmd_prepend);
            break;
        case REQ_CAS:
            INCR(codec_metrics, cmd_cas);
            break;
        case REQ_INCR:
            INCR(codec_metrics, cmd_incr);
            break;
        case REQ_DECR:
            INCR(codec_metrics, cmd_decr);
            break;
        case REQ_STATS:
            INCR(codec_metrics, cmd_stats);
            break;
        case REQ_QUIT:
            INCR(codec_metrics, cmd_quit);
            break;
        default:
            NOT_REACHED();
            break;
        }
    }

done:
    if (req->swallow) {
        parse_swallow(buf);
        request_reset(req);
    }

    return status;
}

/* functions related to composing messages */

#define GET_STRING(_name, _str) str2bstr(_str),
static struct bstring rsp_strings[] = {
    RSP_MSG( GET_STRING )
    null_bstring
};
#undef GET_STRING

static rstatus_t
_compose_rsp_msg(struct buf *buf, rsp_index_t idx)
{
    uint32_t wsize;
    struct bstring *str;

    wsize = buf_wsize(buf);
    str = &rsp_strings[idx];

    if (str->len >= wsize) {
        log_info("failed to write rsp string %d to buf %p: insufficient buffer"
                " space", idx, buf);

        return CC_ENOMEM;
    }

    buf_write_bstring(buf, str);

    log_vverb("wrote rsp string %d to buf %p", idx, buf);

    return CC_OK;
}

rstatus_t
compose_rsp_msg(struct buf *buf, rsp_index_t idx, bool noreply)
{
    rstatus_t status;

    if (noreply) {
        return CC_OK;
    }

    log_verb("rsp msg id %d", idx);
    INCR(codec_metrics, response_compose);

    status = _compose_rsp_msg(buf, idx);
    if (status != CC_OK) {
        INCR(codec_metrics, response_compose_ex);
    }

    return status;
}

static rstatus_t
_compose_rsp_uint64(struct buf *buf, uint64_t val, const char *fmt)
{
    size_t n;
    uint32_t wsize;

    wsize = buf_wsize(buf);

    n = cc_scnprintf(buf->wpos, wsize, fmt, val);
    if (n >= wsize) {
        log_debug("failed to write val %"PRIu64" to buf %p: "
                "insufficient buffer space", val, buf);

        return CC_ENOMEM;
    } else if (n == 0) {
        log_error("failed to write val %"PRIu64" to buf %p: "
                "returned error", val, buf);

        return CC_ERROR;
    }

    log_vverb("wrote rsp uint %"PRIu64" to buf %p", val, buf);

    buf->wpos += n;
    return CC_OK;
}

rstatus_t
compose_rsp_uint64(struct buf *buf, uint64_t val, bool noreply)
{
    rstatus_t status;

    if (noreply) {
        return CC_OK;
    }

    log_verb("rsp int %"PRIu64, val);
    INCR(codec_metrics, response_compose);

    status = _compose_rsp_uint64(buf, val, "%"PRIu64""CRLF);

    if (status != CC_OK) {
        INCR(codec_metrics, response_compose_ex);
    }

    return status;
}

static rstatus_t
_compose_rsp_bstring(struct buf *buf, struct bstring *str)
{
    uint32_t wsize;

    wsize = buf_wsize(buf);

    if (str->len >= wsize) {
        log_info("failed to write bstring %p to buf %p: "
                "insufficient buffer space", str, buf);

        return CC_ENOMEM;
    }

    buf_write_bstring(buf, str);

    log_verb("wrote bstring at %p to buf %p", str, buf);

    return CC_OK;
}

rstatus_t
compose_rsp_keyval(struct buf *buf, struct bstring *key, struct bstring *val, uint32_t flag, uint64_t cas)
{
    rstatus_t status = CC_OK;

    log_verb("rsp keyval: %"PRIu32" byte key, %"PRIu32" byte value,"
            " flag: %"PRIu32", cas: %"PRIu64, key->len, val->len, flag, cas);

    status = _compose_rsp_msg(buf, RSP_VALUE);
    if (status != CC_OK) {
        goto error;
    }

    status = _compose_rsp_bstring(buf, key);
    if (status != CC_OK) {
        goto error;
    }

    status = _compose_rsp_uint64(buf, flag, " %"PRIu64);
    if (status != CC_OK) {
        goto error;
    }

    status = _compose_rsp_uint64(buf, val->len, " %"PRIu64);
    if (status != CC_OK) {
        goto error;
    }

    if (cas) {
        status = _compose_rsp_uint64(buf, cas, " %"PRIu64);
        if (status != CC_OK) {
            goto error;
        }

    }

    status = _compose_rsp_msg(buf, RSP_CRLF);
    if (status != CC_OK) {
        goto error;
    }

    status = _compose_rsp_bstring(buf, val);
    if (status != CC_OK) {
        goto error;
    }

    status = _compose_rsp_msg(buf, RSP_CRLF);
    if (status != CC_OK) {
        goto error;
    }

    INCR(codec_metrics, response_compose);

    return CC_OK;

error:
    INCR(codec_metrics, response_compose_ex);
    return status;
}

static rstatus_t
_compose_rsp_metric(struct buf *buf, struct metric *metric, const char *fmt, ...)
{
    size_t n;
    uint32_t wsize;
    va_list args;

    wsize = buf_wsize(buf);

    va_start(args, fmt);
    n = cc_vscnprintf(buf->wpos, wsize, fmt, args);
    va_end(args);

    if (n >= wsize) {
        log_debug("failed to write metric %s to buf %p: insufficient space",
                metric->name, buf);

        return CC_ENOMEM;
    } else if (n == 0) {
        log_error("failed to write metric %s to buf %p: returned error",
                metric->name, buf);

        return CC_ERROR;
    }

    log_vverb("wrote metric %s to buf %p", metric->name, buf);

    buf->wpos += n;
    return CC_OK;
}

rstatus_t
compose_rsp_stats(struct buf *buf, struct metric marr[], unsigned int nmetric)
{
    unsigned int i;
    rstatus_t status = CC_OK;

    for (i = 0; i < nmetric; i++) {
        switch (marr[i].type) {
        case METRIC_COUNTER:
            status = _compose_rsp_metric(buf, &marr[i], "STAT %s %"PRIu64 CRLF,
                             marr[i].name, marr[i].counter);
            break;

        case METRIC_GAUGE:
            status = _compose_rsp_metric(buf, &marr[i], "STAT %s %"PRIi64 CRLF,
                             marr[i].name, marr[i].gauge);
            break;

        case METRIC_DINTMAX:
            status = _compose_rsp_metric(buf, &marr[i], "STAT %s %ju" CRLF,
                             marr[i].name, marr[i].vintmax);
            break;

        case METRIC_DDOUBLE:
            status = _compose_rsp_metric(buf, &marr[i], "STAT %s %.6f" CRLF,
                             marr[i].name, marr[i].vdouble);
            break;

        default:
            NOT_REACHED();
            break;
        }
        if (status != CC_OK) {
            return status;
            INCR(codec_metrics, response_compose_ex);
        }
    }

    log_verb("wrote %u metrics", nmetric);

    status = _compose_rsp_msg(buf, RSP_END);
    return status;
}
