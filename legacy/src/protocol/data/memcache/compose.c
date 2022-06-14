#include "compose.h"

#include "request.h"
#include "response.h"
#include "time/time.h"

#include <cc_debug.h>
#include <cc_print.h>

#define COMPOSE_MODULE_NAME "protocol::memcache::compose"

#define NOREPLY " noreply"
#define NOREPLY_LEN (sizeof(NOREPLY) - 1)

static bool compose_init = false;
static compose_req_metrics_st *compose_req_metrics = NULL;
static compose_rsp_metrics_st *compose_rsp_metrics = NULL;

void
compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", COMPOSE_MODULE_NAME);

    if (compose_init) {
        log_warn("%s has already been setup, overwrite", COMPOSE_MODULE_NAME);
    }

    compose_req_metrics = req;
    compose_rsp_metrics = rsp;

    compose_init = true;
}

void
compose_teardown(void)
{
    log_info("tear down the %s module", COMPOSE_MODULE_NAME);

    if (!compose_init) {
        log_warn("%s has never been setup", COMPOSE_MODULE_NAME);
    }
    compose_req_metrics = NULL;
    compose_rsp_metrics = NULL;
    compose_init = false;
}

/*
 * common functions
 */

static inline compose_rstatus_e
_check_buf_size(struct buf **buf, uint32_t n)
{
    while (n > buf_wsize(*buf)) {
        if (dbuf_double(buf) != CC_OK) {
            log_debug("failed to write  %u bytes to buf %p: insufficient "
                    "buffer space", n, *buf);

            return COMPOSE_ENOMEM;
        }
    }

    return CC_OK;
}

static inline int
_write_uint64(struct buf **buf, uint64_t val)
{
    size_t n;
    struct buf *b;


    /* NOTE(yao): here we are being conservative on how many bytes we need
     * to print a (64-bit) integer. The actual number might be smaller.
     * But since it is 21 bytes at most (including \0' while buffers usually
     * are KBs in size, it is unlikely to cause many extra expansions.
     */
    if (_check_buf_size(buf, CC_UINT64_MAXLEN) != CC_OK) {
        return COMPOSE_ENOMEM;
    }

    b = *buf;
    /* always succeeds if we have enough space, which we checked above */
    n = cc_print_uint64_unsafe((char *)b->wpos, val);
    b->wpos += n;

    log_vverb("wrote rsp uint %"PRIu64" to buf %p", val, b);

    return n;
}

static inline int
_write_bstring(struct buf **buf, const struct bstring *str)
{
    return buf_write(*buf, str->data, str->len);
}

static inline int
_delim(struct buf **buf)
{
    return buf_write(*buf, " ", 1);
}

static inline int
_crlf(struct buf **buf)
{
    return buf_write(*buf, CRLF, CRLF_LEN);
}

/*
 * request specific functions
 */
static inline int
_noreply(struct buf **buf)
{
    return buf_write(*buf, NOREPLY, NOREPLY_LEN);
}

int
compose_req(struct buf **buf, const struct request *req)
{
    request_type_t type = req->type;
    struct bstring *str = &req_strings[type];
    struct bstring *key = (struct bstring *)req->keys->data;
    int noreply_len = req->noreply * NOREPLY_LEN;
    int cas_len = (req->type == REQ_CAS) ? CC_UINT64_MAXLEN : 0;
    uint32_t i;
    int sz, n = 0;

    switch (type) {
    case REQ_FLUSH:
    case REQ_QUIT:
        if (_check_buf_size(buf, str->len) != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        break;

    case REQ_GET:
    case REQ_GETS:
        for (i = 0, sz = 0; i < array_nelem(req->keys); i++) {
            key = array_get(req->keys, i);
            sz += 1 + key->len;
        }
        if (_check_buf_size(buf, str->len + sz + CRLF_LEN) != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        for (i = 0; i < array_nelem(req->keys); i++) {
            n += _delim(buf);
            n += _write_bstring(buf, (struct bstring *)array_get(req->keys, i));
        }
        n += _crlf(buf);
        break;

    case REQ_DELETE:
        if (_check_buf_size(buf, str->len + key->len + noreply_len + CRLF_LEN)
                != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        n += _write_bstring(buf, key);
        if (req->noreply) {
            n += _noreply(buf);
        }
        n += _crlf(buf);
        break;

    case REQ_SET:
    case REQ_ADD:
    case REQ_REPLACE:
    case REQ_APPEND:
    case REQ_PREPEND:
    case REQ_CAS:
        /* here we may overestimate the size of message header because we
         * estimate the int size based on max value
         */
        if (_check_buf_size(buf, str->len + key->len + CC_UINT32_MAXLEN * 3 +
                    cas_len + req->vstr.len + noreply_len + CRLF_LEN * 2)
                != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        n += _write_bstring(buf, key);
        n += _delim(buf);
        n += _write_uint64(buf, req->flag);
        n += _delim(buf);
        n += _write_uint64(buf, req->expiry);
        n += _delim(buf);
        n += _write_uint64(buf, req->vstr.len);
        if (type == REQ_CAS) {
            n += _delim(buf);
            n += _write_uint64(buf, req->vcas);
        }
        if (req->noreply) {
            n += _noreply(buf);
        }
        n += _crlf(buf);
        n += _write_bstring(buf, &req->vstr);
        n += _crlf(buf);
        break;

    case REQ_INCR:
    case REQ_DECR:
        if (_check_buf_size(buf, str->len + key->len + CC_UINT64_MAXLEN +
                    noreply_len + CRLF_LEN) != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        n += _write_bstring(buf, key);
        n += _delim(buf);
        n += _write_uint64(buf, req->delta);
        if (req->noreply) {
            n += _noreply(buf);
        }
        n += _crlf(buf);
        break;

    default:
        NOT_REACHED();
        break;
    }

    INCR(compose_req_metrics, request_compose);

    return n;

error:
    INCR(compose_req_metrics, request_compose_ex);

    return COMPOSE_ENOMEM;
}

/*
 * response specific functions
 */

int
compose_rsp(struct buf **buf, const struct response *rsp)
{
    int n = 0;
    uint32_t vlen;
    response_type_t type = rsp->type;
    struct bstring *str = &rsp_strings[type];
    int cas_len = rsp->cas * CC_UINT64_MAXLEN;

    /**
     * if we check size for each field to write, we end up being more precise.
     * However, it makes the code really cumbersome to read/write. Instead, we
     * can try to estimate the size for each response upfront and over-estimate
     * length of decimal integers. The absolute margin should be under 40 bytes
     * (2x 32-bit flag+vlen, 1x 64-bit cas) when estimate based on max length.
     * This means in a few cases we will be expanding the buffer unnecessarily,
     * or return error when the message can be squeezed in, but that remains a
     * very small chance in the face of reasonably sized buffers.
     *
     * No delimiter is needed right after each command type (the strings are
     * stored with an extra white space), delimiters are required to be inserted
     * for every additional field.
     */

    log_verb("composing rsp into buf %p from rsp object %p", *buf, rsp);

    switch (type) {
    case RSP_OK:
    case RSP_END:
    case RSP_STORED:
    case RSP_EXISTS:
    case RSP_DELETED:
    case RSP_NOT_FOUND:
    case RSP_NOT_STORED:
        if (_check_buf_size(buf, str->len) != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        log_verb("response type %d, total length %d", rsp->type, n);
        break;

    case RSP_CLIENT_ERROR:
    case RSP_SERVER_ERROR:
        if (_check_buf_size(buf, str->len + rsp->vstr.len + CRLF_LEN) !=
                COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        n += _write_bstring(buf, &rsp->vstr);
        n += _crlf(buf);
        log_verb("response type %d, total length %d", rsp->type, n);
        break;

    case RSP_NUMERIC:
        /* the **_MAXLEN constants include an extra byte for delimiter */
        if (_check_buf_size(buf, CC_UINT64_MAXLEN + CRLF_LEN) != COMPOSE_OK) {
            goto error;
        }
        n += _write_uint64(buf, rsp->vint);
        n += _crlf(buf);
        log_verb("response type %d, total length %d", rsp->type, n);
        break;

    case RSP_VALUE:
        if (rsp->num) {
            vlen = digits(rsp->vint);
        } else {
            vlen = rsp->vstr.len;
        }

        if (_check_buf_size(buf, str->len + rsp->key.len + CC_UINT32_MAXLEN * 2
                    + cas_len + vlen + CRLF_LEN * 2) != COMPOSE_OK) {
            goto error;
        }
        n += _write_bstring(buf, str);
        n += _write_bstring(buf, &rsp->key);
        n += _delim(buf);
        n += _write_uint64(buf, rsp->flag);
        n += _delim(buf);
        n += _write_uint64(buf, vlen);
        if (rsp->cas) {
            n += _delim(buf);
            n += _write_uint64(buf, rsp->vcas);
        }
        n += _crlf(buf);
        if (rsp->num) {
            n += _write_uint64(buf, rsp->vint);
        } else {
            n += _write_bstring(buf, &rsp->vstr);
        }
        n += _crlf(buf);
        log_verb("response type %d, total length %d", rsp->type, n);
        break;

    default:
        NOT_REACHED();
        break;
    }

    INCR(compose_rsp_metrics, response_compose);

    return n;

error:
    INCR(compose_rsp_metrics, response_compose_ex);

    return CC_ENOMEM;
}
