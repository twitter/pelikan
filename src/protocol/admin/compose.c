#include <protocol/admin/compose.h>

#include <protocol/admin/request.h>
#include <protocol/admin/response.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>

#define STAT_MAX_LEN 64 /* metric name <32, value <21 */

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

int
compose_req(struct buf **buf, struct request *req)
{
    struct bstring *str = &req_strings[req->type];
    int n = 0;

    if (_check_buf_size(buf, str->len + req->arg.len + CRLF_LEN) !=
            COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    n += buf_write(*buf, str->data, str->len);
    if (req->arg.len > 0) {
        n += buf_write(*buf, req->arg.data, req->arg.len);
    }
    n += buf_write(*buf, CRLF, CRLF_LEN);

    return n;
}

int
compose_rsp(struct buf **buf, struct response *rsp)
{
    struct bstring *str = &rsp_strings[rsp->type];
    int n = 0;

    if (_check_buf_size(buf, str->len + rsp->data.len) != COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    n += buf_write(*buf, str->data, str->len);
    if (rsp->data.len > 0) {
        n += buf_write(*buf, rsp->data.data, rsp->data.len);
    }

    return n;
}
