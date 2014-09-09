#include <bb_response.h>

#include <cc_print.h>
#include <cc_util.h>

#define GET_STRING(_name, _str) str2bstr(_str),
static struct bstring rsp_strings[] = {
    RSP_MSG( GET_STRING )
    null_bstring
};
#undef GET_STRING


rstatus_t
rsp_write_msg(struct mbuf *buf, rsp_index_t idx)
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
rsp_write_uint64(struct mbuf *buf, uint64_t val)
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
rsp_write_bstring(struct mbuf *buf, struct bstring *str)
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

