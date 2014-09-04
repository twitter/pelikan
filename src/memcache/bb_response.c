#include <bb_response.h>

#include <cc_util.h>

static rstatus_t
_rsp_write_str(struct mbuf *buf, rsp_index_t rsp_idx);
    uint32_t wsize;
    struct bstring &str;

    wsize = mbuf_wsize(buf);
    str = &rsp_strings[rsp_idx];

    if (str->len >= wsize) {
        log_debug(LOG_INFO, "failed to write rsp string %d to mbuf %p: "
                "insufficient buffer space", rsp_idx, buf);

        return CC_ENOMEM;
    }

    mbuf_copy(buf, str->data, str->len);
    buf->wpos += str->len;

    log_debug(LOG_VVERB, "wrote rsp string %d to mbuf %p", rsp_idx, buf);

    return CC_OK;
}

static size_t
_rsp_print_uint64(char num_str[], uint64_t val)
{
    size_t n;
    n = sc_scnprintf(num_str, CC_UINT64_MAXLEN, "%"PRIu64, val);

    return n;
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
    } else if (n <= 0) {
        log_debug(LOG_NOTICE, "failed to write val %"PRIu64" to mbuf %p: "
                "returned error %d", val, buf, n);

        return CC_ERROR;
    }

    buf->wpos += n;
    return CC_OK;
}
