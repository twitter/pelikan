#include <protocol/admin/compose.h>

#include <protocol/admin/op.h>
#include <protocol/admin/reply.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>

#define STAT_MAX_LEN 1024       /* max length of a single stat */

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
    op_type_t type = op->type;
    struct bstring *str = &op_strings[type];
    int n = 0;

    switch (type) {
    case OP_STATS:
    case OP_VERSION:
    case OP_QUIT:
        if (_check_buf_size(buf, str->len + CRLF_LEN) != COMPOSE_OK) {
            return COMPOSE_ENOMEM;
        }
        n += buf_write(*buf, str->data, str->len);
        n += buf_write(*buf, CRLF, CRLF_LEN);
        break;
    default:
        NOT_REACHED();
        break;
    }

    return n;
}

int
compose_rep(struct buf **buf, struct reply *rep)
{
    int n = 0;
    reply_type_t type = rep->type;
    struct bstring stat_str, *str = &reply_strings[type];
    char stat_buf[STAT_MAX_LEN];

    switch (type) {
    case REP_STAT:
        stat_str.len = metric_print(stat_buf, STAT_MAX_LEN, rep->met);
        if (stat_str.len == 0) {
            return COMPOSE_EOVERSIZED;
        }
        stat_str.data = stat_buf;
        if (_check_buf_size(buf, str->len + stat_str.len + CRLF_LEN) !=
            COMPOSE_OK) {
            return COMPOSE_ENOMEM;
        }
        n += buf_write(*buf, str->data, str->len);
        n += buf_write(*buf, stat_str.data, stat_str.len);
        n += buf_write(*buf, CRLF, CRLF_LEN);
        break;
    case REP_VERSION:
    case REP_CLIENT_ERROR:
    case REP_SERVER_ERROR:
        if (_check_buf_size(buf, str->len + rep->vstr.len + CRLF_LEN) !=
            COMPOSE_OK) {
            return COMPOSE_ENOMEM;
        }
        n += buf_write(*buf, str->data, str->len);
        n += buf_write(*buf, rep->vstr.data, rep->vstr.len);
        n += buf_write(*buf, CRLF, CRLF_LEN);
        break;
    case REP_END:
        if (_check_buf_size(buf, str->len) != COMPOSE_OK) {
            return COMPOSE_ENOMEM;
        }
        n += buf_write(*buf, str->data, str->len);
        break;
    default:
        NOT_REACHED();
        break;
    }

    return n;
}
