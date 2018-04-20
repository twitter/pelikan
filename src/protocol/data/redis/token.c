#include "token.h"

#include "request.h"
#include "response.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_define.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>

#define STR_MAXLEN 255 /* max length for simple string or error */
#define BULK_MAXLEN (512 * MiB)
#define ARRAY_MAXLEN (64 * MiB)

#define NIL_STR "$-1\r\n"


static inline compose_rstatus_t
_check_buf_size(struct buf **buf, uint32_t n)
{
    while (n > buf_wsize(*buf)) {
        if (dbuf_double(buf) != CC_OK) {
            log_debug("failed to write  %u bytes to buf %p: insufficient "
                    "buffer space", n, *buf);

            return COMPOSE_ENOMEM;
        }
    }

    return COMPOSE_OK;
}


static parse_rstatus_t
_read_str(struct bstring *str, struct buf *buf)
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    str->len = 0;
    str->data = buf->rpos;
    /*
     * Note: according to @antirez, simple strings are not supposed to be empty.
     * However, there's no particular harm allowing a null simple string, so
     * we allow it in this function
     */
    for (; buf->rpos < buf->wpos; buf->rpos++) {
        if (line_end(buf)) {
            buf->rpos += CRLF_LEN;
            log_vverb("simple string detected at %p, length %"PRIu32, str->len);

            return PARSE_OK;
        }
        if (++str->len > STR_MAXLEN) {
            log_warn("simple string max length (%d) exceeded", STR_MAXLEN);

            return PARSE_EOVERSIZE;
        }
    }

    return PARSE_EUNFIN;
}


static parse_rstatus_t
_read_int(int64_t *num, struct buf *buf, int64_t min, int64_t max)
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */
    size_t len = 0;
    int64_t sign = 1;

    if (*buf->rpos == '-') {
        sign = -1;
        buf->rpos++;
    }

    *num = 0;
    for (; buf_rsize(buf) > 0; buf->rpos++) {
        if (isdigit(*buf->rpos)) {
            if (*num < min / 10 || *num > max / 10) {
                /* TODO(yao): catch the few numbers that will still overflow */
                log_warn("ill formatted token: integer out of bounds");

                return PARSE_EOVERSIZE;
            }

            len++;
            *num = *num * 10ULL + sign * (*buf->rpos - '0');
        } else {
            if (len == 0 || *buf->rpos != CR) {
                log_warn("invalid character encountered: %c", *buf->rpos);

                return PARSE_EINVALID;
            }
            if (line_end(buf)) {
                buf->rpos += CRLF_LEN;
                log_vverb("parsed integer, value %"PRIi64, *num);

                return PARSE_OK;
            } else {
                return PARSE_EUNFIN;
            }
        }
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_t
_read_bulk(struct bstring *str, struct buf *buf)
{
    parse_rstatus_t status;
    int64_t len;

    bstring_init(str);
    status = _read_int(&len, buf, -1, BULK_MAXLEN);
    if (status != PARSE_OK) {
        return status;
    }
    if (len < 0) {
        log_vverb("null bulk string detected at %p", buf->rpos);

        return PARSE_EEMPTY;
    }

    if (buf_rsize(buf) >= len + CRLF_LEN) {
        /* have enough bytes for the whole payload plus CRLF */
        str->len = len;
        str->data = buf->rpos;
        buf->rpos += str->len;

        if (line_end(buf)) {
            buf->rpos += CRLF_LEN;
            log_vverb("bulk string detected at %p, length %"PRIu32, buf->rpos,
                    len);

            return PARSE_OK;
        } else {
            if (*buf->rpos == CR) {
                return PARSE_EUNFIN;
            }

            log_warn("invalid character encountered, expecting CRLF: %c%c",
                    *buf->rpos, *(buf->rpos + 1));

            return PARSE_EINVALID;
        }
    }

    return PARSE_EUNFIN;
}

static inline int
_write_int(struct buf *buf, int64_t val)
{
    size_t n = 0;

    n = cc_print_int64_unsafe(buf->wpos, val);
    buf->wpos += n;

    buf_write(buf, CRLF, CRLF_LEN);

    return (n + CRLF_LEN);
}

static inline int
_write_bstr(struct buf *buf, struct bstring *bstr)
{
    buf_write(buf, bstr->data, bstr->len);
    buf_write(buf, CRLF, CRLF_LEN);

    return (bstr->len + CRLF_LEN);
}


bool
token_is_array(struct buf *buf)
{
    ASSERT(buf != NULL);

    return *(buf->rpos) == '*';
}

parse_rstatus_t
token_array_nelem(int64_t *nelem, struct buf *buf)
{
    parse_rstatus_t status;
    char *pos;

    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_array(buf));

    pos = buf->rpos++;
    status = _read_int(nelem, buf, -1, ARRAY_MAXLEN);
    if (status == PARSE_EUNFIN) {
        buf->rpos = pos;
    }

    return status;
}


/* this function does not handle array, which is a composite type */
parse_rstatus_t
parse_element(struct element *el, struct buf *buf)
{
    char *p;
    parse_rstatus_t status;

    ASSERT(buf_rsize(buf) > 0);

    log_verb("detecting the next element %p in buf %p", el, buf);

    p = buf->rpos++;
    switch (*p) {
    case '+':
        /* simple string */
        el->type = ELEM_STR;
        status = _read_str(&el->bstr, buf);
        break;

    case '-':
        /* error */
        el->type = ELEM_ERR;
        status = _read_str(&el->bstr, buf);
        break;

    case ':':
        /* integer */
        el->type = ELEM_INT;
        status = _read_int(&el->num, buf, INT64_MIN, INT64_MAX);
        break;

    case '$':
        /* bulk string */
        el->type = ELEM_BULK;
        status = _read_bulk(&el->bstr, buf);
        if (status == PARSE_EEMPTY) {
            status = PARSE_OK;
            el->type = ELEM_NIL;
        }
        break;

    default:
        return PARSE_EINVALID;
    }

    if (status != PARSE_OK) { /* rewind */
        buf->rpos = p;
    }

    return status;
}


int
compose_array_header(struct buf **buf, int nelem)
{
    struct buf *b;
    size_t n = 1 + CRLF_LEN + CC_INT64_MAXLEN;

    if (_check_buf_size(buf, n) != COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    b = *buf;
    *b->wpos++ = '*';
    return (1 + _write_int(b, nelem));
}

/* this function does not handle array, which is a composite type */
int
compose_element(struct buf **buf, struct element *el)
{
    size_t n = 1 + CRLF_LEN;
    struct buf *b;

    ASSERT(el->type > 0);

    /* estimate size (overestimate space needed for integers (int, bulk)) */
    switch (el->type) {
    case ELEM_STR:
    case ELEM_ERR:
        n += el->bstr.len;
        break;

    case ELEM_INT:
        n += CC_INT64_MAXLEN;
        break;

    case ELEM_BULK:
        n += el->bstr.len + CC_INT64_MAXLEN + CRLF_LEN;
        break;

    case ELEM_NIL:
        n += 2; /* "-1" */
        break;

    default:
        return COMPOSE_EINVALID;
    }

    if (_check_buf_size(buf, n) != COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    b = *buf;
    log_verb("write element %p in buf %p", el, b);

    switch (el->type) {
    case ELEM_STR:
        n = buf_write(b, "+", 1);
        n += _write_bstr(b, &el->bstr);
        break;

    case ELEM_ERR:
        n = buf_write(b, "-", 1);
        n += _write_bstr(b, &el->bstr);
        break;

    case ELEM_INT:
        n = buf_write(b, ":", 1);
        n += _write_int(b, el->num);
        break;

    case ELEM_BULK:
        n = buf_write(b, "$", 1);
        n += _write_int(b, el->bstr.len);
        n += _write_bstr(b, &el->bstr);
        break;

    case ELEM_NIL:
        n = sizeof(NIL_STR) - 1;
        buf_write(b, NIL_STR, n);
        break;

    default:
        NOT_REACHED();
    }

    return n;
}
