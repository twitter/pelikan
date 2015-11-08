#include <protocol/admin/compose.h>

#include <protocol/admin/op.h>
#include <protocol/admin/reply.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>

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
compose_rep(struct buf **buf, struct reply *rep)
{
    int n = 0;
    reply_type_t type = rep->type;
    struct bstring *str = &reply_strings[type];

    switch (type) {
    case REP_OK:
        if (_check_buf_size(buf, str->len) != COMPOSE_OK) {
            return COMPOSE_ENOMEM;
        }
        n += buf_write(*buf, str->data, str->len);
        break;
    case REP_STAT:
        n += buf_write(*buf, "stats not implemented" CRLF,
                       sizeof("stats not implemented") + CRLF_LEN - 1);
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
    default:
        NOT_REACHED();
        break;
    }

    return n;
}
