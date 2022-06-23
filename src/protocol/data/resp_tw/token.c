#include "token.h"

#include "request.h"
#include "response.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_define.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>
#include <errno.h>
#include <math.h>

#define STR_MAXLEN 255 /* max length for simple string or error */
#define BULK_MAXLEN (512 * MiB)
#define ARRAY_MAXLEN (64 * MiB)
#define BIGNUM_MAXLEN STR_MAXLEN

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

    return COMPOSE_OK;
}

static inline parse_rstatus_e
_try_match_inner(const char* match, size_t match_len, struct buf *buf) {
    if (buf_rsize(buf) < match_len) {
        return PARSE_EUNFIN;
    }

    for (size_t i = 0; i < match_len; ++i) {
        if (buf->rpos[i] != match[i]) {
            return PARSE_EINVALID;
        }
    }

    buf->rpos += match_len;

    return PARSE_OK;
}

/*=================================================================
 * REDS3 Parsing Functions
 *=================================================================
 */

/*
 * Attempts to match the given sequence of characters in the input.
 * Does not match the final nul character. 
 * 
 * Note that match must resolve to a string literal.
 * 
 * Return values:
 *   - PARSE_EINVALID if the string does not match
 *   - PARSE_EUNFIN if there is not enough buffer to match the string
 *   - PARSE_OK otherwise
 *  * This function updates buf->rpos only when it returns PARSE_OK.
 */
#define _try_match(match, buf) _try_match_inner((match), sizeof(match) - 1, buf)

static parse_rstatus_e
_read_crlf(struct buf *buf) 
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    if (buf_rsize(buf) >= CRLF_LEN) {
        if (!line_end(buf)) {
            log_warn("invalid character encountered, expecting CRLF: %c%c",
                    buf->rpos[0], buf->rpos[1]);

            return PARSE_EINVALID;
        }

        buf->rpos += CRLF_LEN;
        return PARSE_OK;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_e
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
    for (; buf_rsize(buf) > 0; buf->rpos++) {
        if (line_end(buf)) {
            buf->rpos += CRLF_LEN;
            log_vverb("simple string detected at %p, length %"PRIu32, buf->rpos, str->len);

            return PARSE_OK;
        }
        if (++str->len > STR_MAXLEN) {
            log_warn("simple string max length (%d) exceeded", STR_MAXLEN);

            return PARSE_EOVERSIZE;
        }
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_e
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
                if (*num < min || *num > max) {
                    log_warn("ill formatted token: integer out of bounds");

                    return PARSE_EOVERSIZE;
                }

                buf->rpos += CRLF_LEN;
                log_vverb("parsed integer, value %" PRIi64, *num);

                return PARSE_OK;
            } else {
                return PARSE_EUNFIN;
            }
        }
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_e
_read_uint(uint64_t *num, struct buf *buf, uint64_t max) 
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    size_t len = 0;

    *num = 0;
    for (; buf_rsize(buf) > 0; buf->rpos++) {
        if (isdigit(*buf->rpos)) {
            uint64_t digit = (*buf->rpos - '0');

            if (*num > max / 10) {
                log_warn("ill formatted token: integer out of bounds");

                return PARSE_EOVERSIZE;
            }

            len++;
            *num = *num * 10ULL + digit;
        } else {
            if (len == 0 || *buf->rpos != CR) {
                log_warn("invalid character encountered: %c", *buf->rpos);

                return PARSE_EINVALID;
            }
            if (line_end(buf)) {
                if (*num > max) {
                    /* Note: This ensures that we are actually in bounds */
                    log_warn("ill formatted token: integer out of bounds");

                    return PARSE_EOVERSIZE;
                }

                buf->rpos += CRLF_LEN;
                log_vverb("parsed integer, value %" PRIu64, *num);

                return PARSE_OK;
            }

            return PARSE_EUNFIN;
        }
    }
    
    return PARSE_EUNFIN;
}

static parse_rstatus_e
_read_blob(struct bstring *str, struct buf *buf)
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    parse_rstatus_e status;
    uint64_t len;

    bstring_init(str);
    status = _read_uint(&len, buf, BULK_MAXLEN);
    if (status != PARSE_OK) {
        return status;
    }

    if (buf_rsize(buf) >= len + CRLF_LEN) {
        /* have enough bytes for the whole payload plus CRLF */
        str->len = len;
        str->data = buf->rpos;
        buf->rpos += str->len;

        if (line_end(buf)) {
            buf->rpos += CRLF_LEN;
            log_vverb("bulk string detected at %p, length %" PRIu32, buf->rpos, len);

            return PARSE_OK;
        }

        if (*buf->rpos == CR) {
            return PARSE_EUNFIN;
        }

        log_warn("invalid character encountered, expecting CRLF: %c%c",
                *buf->rpos, *(buf->rpos + 1));
        
        return PARSE_EINVALID;
    }

    return PARSE_EUNFIN;
}

static parse_rstatus_e
_read_nil(struct buf *buf)
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    /* Note: all this does is validate the CRLF */
    char* old_rpos = buf->rpos;
    parse_rstatus_e status = _read_crlf(buf);
    
    if (status == PARSE_OK) {
        log_vverb("nil detected at %p", old_rpos);
    }

    return status;
}

static parse_rstatus_e
_read_bool(bool *val, struct buf *buf)
{
    /*
     * Note: buf->rpos is updated in this function, the caller is responsible
     * for resetting the pointer if necessary.
     */

    if (buf_rsize(buf) < CRLF_LEN + 1) {
        return PARSE_EUNFIN;
    }

    switch (*buf->rpos) {
    case 't': 
        *val = true; 
        break;
    case 'f':
        *val = false;
        break;
    default:
        log_warn("invalid character encountered, expected t or f: %c", *buf->rpos);
        return PARSE_EINVALID;
    }

    buf->rpos++;
    parse_rstatus_e status = _read_crlf(buf);

    if (status == PARSE_OK) {
        log_vverb("parsed boolean, value %c", *val ? 't' : 'f');
    }

    return status;
}

/* Parses a double according to the reds3 specification.
 * 
 * According to the REDS3 spec, a double can be any of
 *   - A number (e.g. '10')
 *   - A number with a decimal point in the middle (e.g. '0.121' or
 *     '1241.1' but not '.5')
 *   - inf
 * Any of these forms can be preceded by a minus sign ('-') for a negative
 * number.
 * 
 * The specification does not detail what should happen when the number
 * would underflow or overflow, so this implementation makes the following
 * choices:
 *   - If the value is too large to be represented as a double (overflow)
 *     then we error out with PARSE_EINVALID
 *   - If the value is too small to be represented as a double (underflow)
 *     then we round to 0
 * 
 * Beyond that, as we use strtod for parsing any specifics such as whether
 * '-0.0' is stored as a signed 0 is up to the C stdlib implementation.
 * 
 * Note: buf->rpos is updated in this function, the caller is responsible
 * for resetting the pointer if necessary.
 */
static parse_rstatus_e
_read_double(double *val, struct buf *buf)
{
    parse_rstatus_e status = 0;
    size_t len = 0;
    char* start = buf->rpos;

    /* Check for all different literals that REDS3 supports.
     * These are inf and -inf
     */

    status = _try_match("inf\r\n", buf);
    if (status == PARSE_EUNFIN) {
        return PARSE_EUNFIN;
    } else if (status == PARSE_OK) {
        *val = 1.0 / 0.0;
        
        log_vverb("parsed double, value inf");
        return PARSE_OK;
    }

    status = _try_match("-inf\r\n", buf);
    if (status == PARSE_EUNFIN) {
        return PARSE_EUNFIN;
    } else if (status == PARSE_OK) {
        *val = -1.0 / 0.0;

        log_vverb("parsed double, value -inf");
        return PARSE_OK;
    }

    for (; buf_rsize(buf) > 0; ++buf->rpos, ++len) {
        if (*buf->rpos == CR) {
            break;
        }

        if (!isdigit(*buf->rpos) && !(*buf->rpos == '.') && !(*buf->rpos == '-')) {
            log_warn("invalid character encountered: %c", *buf->rpos);
            return PARSE_EINVALID;
        }
    }

    /* Need to ensure that there is a character present after the
     * number otherwise strtod could read beyond the buffer.
     */
    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    } else if (len == 0) {
        log_warn("ill formatted token: empty double");
        return PARSE_EEMPTY;
    }

    /* According to the spec a double of the form '.102' is invalid */
    if (*start == '.') {
        log_warn("ill formatted token: double starting with '.'");
        return PARSE_EINVALID;
    }

    char* end;
    errno = 0;
    *val = strtod(start, &end);

    if (errno == ERANGE) {
        /* TODO(sean): Should large doubles be rounded to infinity? */
        if (*val == HUGE_VAL || *val == -HUGE_VAL) {
            log_warn("ill formatted token: double was out of range");
            return PARSE_EINVALID;
        }
        if (*val == 0.0) {
            /*
             * This implementation assumes that doubles which are too
             * small can safely be flushed to 0.
             */
        }
    }

    log_vverb("pased double, value was %f", *val);
    return PARSE_OK;
}

/* Parse a big integer according to the REDS3 specification.
 * 
 * Note: buf->rpos is updated in this function, the caller is responsible
 * for resetting the pointer if necessary.
 */
static parse_rstatus_e
_read_big_number(struct bstring *str, struct buf *buf) {
    bstring_init(str);

    str->len = 0;
    str->data = buf->rpos;

    for (; buf_rsize(buf) > 0; buf->rpos++) {
        if (line_end(buf)) {
            buf->rpos += CRLF_LEN;
            log_vverb("big number detected at %p, length %" PRIu32, buf->rpos, str->len);

            return PARSE_OK;
        }

        if (!isdigit(*buf->rpos)) {
            log_warn("big number contained invalid character: %c", *buf->rpos);
            return PARSE_EINVALID;
        }

        if (++str->len > BIGNUM_MAXLEN) {
            log_warn("big number max length (%d) exceeded", BIGNUM_MAXLEN);
            return PARSE_EOVERSIZE;
        }
    }

    return PARSE_EUNFIN;
}

/* Parse a single value. This does not handle any composite
 * types such as arrays, sets, maps, push data, or 
 * associated data.
 */
parse_rstatus_e
parse_element(struct element *el, struct buf *buf)
{
    char *p;
    parse_rstatus_e status;

    log_verb("detecting the next element %p in buf %p", el, buf);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    p = buf->rpos++;
    switch (*p) {
    case '+':
        /* simple string */
        el->type = ELEM_STR;
        status = _read_str(&el->bstr, buf);
        break;
    
    case '-':
        /* simple error */
        el->type = ELEM_ERR;
        status = _read_str(&el->bstr, buf);
        break;

    case '$':
        /* blob string */
        el->type = ELEM_BLOB_STR;
        status = _read_blob(&el->bstr, buf);
        break;

    case '!':
        /* blob error */
        el->type = ELEM_BLOB_ERR;
        status = _read_blob(&el->bstr, buf);
        break;

    case '=':
        /* verbatim string */
        el->type = ELEM_VERBATIM_STR;
        status = _read_blob(&el->bstr, buf);

        /* Verbatim strings are like bulk strings with the extra
         * requirement that they start with 3 bytes that identify
         * the type followed by a colon.
         */
        if (!(el->bstr.len > 4 && el->bstr.data[3] == ':')) {
            log_warn("invalid verbatim string, did not start with type marker");
            status = PARSE_EINVALID;
        }

        break;

    case ':':
        /* number */
        el->type = ELEM_NUMBER;
        status = _read_int(&el->num, buf, INT64_MIN, INT64_MAX);
        break;

    case ',':
        log_warn("found unsupported double in message");
        return PARSE_ENOTSUPPORTED;
        /* double */
        el->type = ELEM_DOUBLE;
        status = _read_double(&el->double_, buf);
        break;

    case '(':
        log_warn("found unsupported big number in message");
        return PARSE_ENOTSUPPORTED;
        /* big number */
        el->type = ELEM_BIG_NUMBER;
        status = _read_big_number(&el->bstr, buf);
        break;

    case '_':
        /* nil */
        el->type = ELEM_NIL;
        status = _read_nil(buf);
        break;

    case '#':
        /* bool */
        el->type = ELEM_BOOL;
        status = _read_bool(&el->boolean, buf);
        break;

    default:
        log_warn("'%c' is not a valid single-element type header", *p);
        return PARSE_EINVALID;
    }

    if (status != PARSE_OK) { /* rewind */
        buf->rpos = p;
    }

    return status;
}

static inline parse_rstatus_e
_token_generic_nelem(uint64_t *nelem, struct buf *buf) {

    char *pos = buf->rpos++;
    parse_rstatus_e status = _read_uint(nelem, buf, ARRAY_MAXLEN);
    if (status == PARSE_EUNFIN) {
        buf->rpos = pos;
    }

    return status;
}

parse_rstatus_e
token_array_nelem(uint64_t *nelem, struct buf *buf)
{
    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_array(buf));

    return _token_generic_nelem(nelem, buf);
}
parse_rstatus_e
token_map_nelem(uint64_t *nelem, struct buf *buf)
{
    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_map(buf));

    parse_rstatus_e status = _token_generic_nelem(nelem, buf);
    if (status == PARSE_OK) {
        /* need to read both keys and values */
        *nelem *= 2;
    }

    return status;
}
parse_rstatus_e
token_set_nelem(uint64_t *nelem, struct buf *buf)
{
    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_set(buf));

    return _token_generic_nelem(nelem, buf);
}
parse_rstatus_e
token_attribute_nelem(uint64_t *nelem, struct buf *buf)
{
    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_attribute(buf));

    return _token_generic_nelem(nelem, buf);
}
parse_rstatus_e
token_push_data_nelem(uint64_t *nelem, struct buf *buf)
{
    ASSERT(nelem != NULL && buf != NULL);
    ASSERT(token_is_push_data(buf));

    return _token_generic_nelem(nelem, buf);
}

/*=================================================================
 * Composite Type Identification Functions
 *=================================================================
 */

bool
token_is_array(struct buf *buf)
{
    ASSERT(buf != NULL);

    return buf_rsize(buf) > 0 && *buf->rpos == '*';
}

bool
token_is_map(struct buf *buf)
{
    ASSERT(buf != NULL);

    return buf_rsize(buf) > 0 && *buf->rpos == '%';
}

bool
token_is_set(struct buf *buf)
{
    ASSERT(buf != NULL);

    return buf_rsize(buf) > 0 && *buf->rpos == '~';
}

bool
token_is_attribute(struct buf *buf) 
{
    ASSERT(buf != NULL);

    return buf_rsize(buf) > 0 && *buf->rpos == '|';
}

bool
token_is_push_data(struct buf *buf)
{
    ASSERT(buf != NULL);

    return buf_rsize(buf) > 0 && *buf->rpos == '>';
}

/*=================================================================
 * REDS3 Protocol Composition Functions
 *=================================================================
 */

#define _write_lit(buf, lit) buf_write(buf, lit, sizeof(lit) - 1)

static inline size_t
_write_uint(struct buf *buf, uint64_t val)
{
    size_t n = cc_print_uint64_unsafe(buf->wpos, val);
    buf->wpos += n;
    return n;
}

static inline size_t
_write_int(struct buf *buf, int64_t val)
{
    size_t n = cc_print_int64_unsafe(buf->wpos, val);
    buf->wpos += n;
    return n;
}

static inline size_t
_write_bstr(struct buf *buf, struct bstring *bstr)
{
    return buf_write(buf, bstr->data, bstr->len);
}

static inline size_t
_write_bool(struct buf *buf, bool val)
{
    if (val) {
        return _write_lit(buf, "t");
    } else {
        return _write_lit(buf, "f");
    }
}

static inline size_t
_write_blob(struct buf *buf, struct bstring *bstr)
{
    size_t n = 0;
    n += _write_uint(buf, bstr->len);
    n += _write_lit(buf, CRLF);
    n += _write_bstr(buf, bstr);
    return n;
}

static inline int
_compose_header_generic(struct buf **buf, uint64_t nelem, char val)
{
    struct buf *b;
    size_t n = 1 + CRLF_LEN + CC_UINT64_MAXLEN;

    if (_check_buf_size(buf, n) != COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    b = *buf;
    *b->wpos++ = val;

    int len = 1;
    len += _write_int(b, nelem);
    len += _write_lit(b, CRLF);
    return len;
}

int
compose_array_header(struct buf **buf, uint64_t nelem)
{
    return _compose_header_generic(buf, nelem, '*');
}
int
compose_map_header(struct buf **buf, uint64_t nelem)
{
    if (nelem % 2 != 0) {
        log_warn("tried to create a map with an odd number of elements (%" PRIu64 " elements)", 
                nelem);
        return COMPOSE_EINVALID;
    }

    return _compose_header_generic(buf, nelem/2, '%');
}
int
compose_set_header(struct buf **buf, uint64_t nelem)
{
    return _compose_header_generic(buf, nelem, '~');
}
int
compose_attribute_header(struct buf **buf, uint64_t nelem)
{
    return _compose_header_generic(buf, nelem, '|');
}
int
compose_push_data_header(struct buf **buf, uint64_t nelem)
{
    return _compose_header_generic(buf, nelem, '>');
}

int
compose_element(struct buf **buf, struct element *el)
{
    size_t n = 1 + CRLF_LEN;
    struct buf *b;

    ASSERT(el->type > 0);

    /* estimate size (overestimages the size for anything that serializes an integer) */
    switch (el->type) {
    case ELEM_STR:
    case ELEM_ERR:
        n += el->bstr.len;
        break;

    case ELEM_NUMBER:
        n += CC_UINT64_MAXLEN;
        break;

    case ELEM_BLOB_STR:
    case ELEM_BLOB_ERR:
    case ELEM_VERBATIM_STR:
        n += el->bstr.len + CC_UINT64_MAXLEN + CRLF_LEN;

    case ELEM_NIL:
        break;
    
    case ELEM_BOOL:
        n += 1;
        break;

    case ELEM_DOUBLE:
    case ELEM_BIG_NUMBER:
        return COMPOSE_ENOTSUPPORTED;

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
        n = _write_lit(b, "+");
        n += _write_bstr(b, &el->bstr);
        break;

    case ELEM_ERR:
        n = _write_lit(b, "-");
        n += _write_bstr(b, &el->bstr);
        break;

    case ELEM_BLOB_STR:
        n = _write_lit(b, "$");
        n += _write_blob(b, &el->bstr);
        break;

    case ELEM_BLOB_ERR:
        n = _write_lit(b, "!");
        n += _write_blob(b, &el->bstr);
        break;

    case ELEM_VERBATIM_STR:
        n = _write_lit(b, "=");
        n += _write_blob(b, &el->bstr);
        break;

    case ELEM_NUMBER:
        n = _write_lit(b, ":");
        n += _write_int(b, el->num);
        break;

    case ELEM_NIL:
        n = _write_lit(b, "_");
        break;
    
    case ELEM_BOOL:
        n = _write_lit(b, "#");
        n += _write_bool(b, el->boolean);
        break;

    default:
        NOT_REACHED();
    }

    n += _write_lit(b, CRLF);
    /* If n > INT_MAX then the conversion here would cause UB */
    ASSERT(n < INT_MAX);

    return n;
}
