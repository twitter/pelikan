#include <protocol/admin/compose.h>

#include <protocol/admin/op.h>
#include <protocol/admin/reply.h>

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
compose_op(struct buf **buf, struct op *op)
{
    struct bstring *str = &op_strings[op->type];
    int n = 0;

    if (_check_buf_size(buf, str->len + op->arg.len + CRLF_LEN) !=
            COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    n += buf_write(*buf, str->data, str->len);
    if (op->arg.len > 0) {
        n += buf_write(*buf, op->arg.data, op->arg.len);
    }
    n += buf_write(*buf, CRLF, CRLF_LEN);

    return n;
}

int
compose_rep(struct buf **buf, struct reply *rep)
{
    struct bstring *str = &reply_strings[rep->type];
    int n = 0;

    if (_check_buf_size(buf, str->len + rep->data.len) != COMPOSE_OK) {
        return COMPOSE_ENOMEM;
    }

    n += buf_write(*buf, str->data, str->len);
    if (rep->data.len > 0) {
        n += buf_write(*buf, rep->data.data, rep->data.len);
    }

    return n;
}
