#include <memcache/bb_response.h>

#include <cc_print.h>
#include <cc_util.h>

#define GET_STRING(_name, _str) str2bstr(_str),
static struct bstring rsp_strings[] = {
    RSP_MSG( GET_STRING )
    null_bstring
};
#undef GET_STRING


static rstatus_t
_rsp_write_msg(struct mbuf *buf, rsp_index_t idx)
{
    uint32_t wsize;
    struct bstring *str;

    wsize = mbuf_wsize(buf);
    str = &rsp_strings[idx];

    if (str->len >= wsize) {
        log_debug(LOG_INFO, "failed to write rsp string %d to mbuf %p: "
                "insufficient buffer space", idx, buf);

        return CC_ENOMEM;
    }

    mbuf_copy(buf, str->data, str->len);
    buf->wpos += str->len;

    log_debug(LOG_VVERB, "wrote rsp string %d to mbuf %p", idx, buf);

    return CC_OK;
}

rstatus_t
rsp_write_msg(struct mbuf *buf, rsp_index_t idx, bool noreply)
{
    if (noreply) {
        return CC_OK;
    }

    return _rsp_write_msg(buf, idx);
}

static rstatus_t
_rsp_write_uint64(struct mbuf *buf, uint64_t val)
{
    size_t n;
    uint32_t wsize;

    wsize = mbuf_wsize(buf);

    n = cc_scnprintf(buf->wpos, wsize, "%"PRIu64, val);
    if (n >= wsize) {
        log_debug(LOG_INFO, "failed to write val %"PRIu64" to mbuf %p: "
                "insufficient buffer space", val, buf);

        return CC_ENOMEM;
    } else if (n == 0) {
        log_debug(LOG_NOTICE, "failed to write val %"PRIu64" to mbuf %p: "
                "returned error", val, buf);

        return CC_ERROR;
    }

    buf->wpos += n;
    return CC_OK;
}

rstatus_t
rsp_write_uint64(struct mbuf *buf, uint64_t val, bool noreply)
{
    if (noreply) {
        return CC_OK;
    }

    return _rsp_write_uint64(buf, val);
}

static rstatus_t
_rsp_write_bstring(struct mbuf *buf, struct bstring *str)
{
    uint32_t wsize;

    wsize = mbuf_wsize(buf);

    if (str->len >= wsize) {
        log_debug(LOG_INFO, "failed to write bstring %p to mbuf %p: "
                "insufficient buffer space", str, buf);

        return CC_ENOMEM;
    }

    mbuf_copy(buf, str->data, str->len);
    buf->wpos += str->len;

    log_debug(LOG_VVERB, "wrote bstring %p to mbuf %p", str, buf);

    return CC_OK;
}

rstatus_t
rsp_write_bstring(struct mbuf *buf, struct bstring *str, bool noreply)
{
    if (noreply) {
        return CC_OK;
    }

    return _rsp_write_bstring(buf, str);
}

rstatus_t
rsp_write_keyval(struct mbuf *buf, struct bstring *key, struct bstring *val, uint32_t flag, uint64_t cas)
{
    rstatus_t status = CC_OK;

    status = _rsp_write_msg(buf, RSP_VALUE);
    if (status != CC_OK) {
        return status;
    }

    status = _rsp_write_bstring(buf, key);
    if (status != CC_OK) {
        return status;
    }

    status = _rsp_write_uint64(buf, flag);
    if (status != CC_OK) {
        return status;
    }

    status = _rsp_write_uint64(buf, val->len);
    if (status != CC_OK) {
        return status;
    }

    if (cas) {
        status = _rsp_write_uint64(buf, cas);
        if (status != CC_OK) {
            return status;
        }

    }

    status = _rsp_write_msg(buf, RSP_CRLF);
    if (status != CC_OK) {
        return status;
    }

    status = _rsp_write_bstring(buf, val);
    if (status != CC_OK) {
        return status;
    }

    status = _rsp_write_bstring(buf, RSP_CRLF);
    if (status != CC_OK) {
        return status;
    }

    return status;
}
