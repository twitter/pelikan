#include <protocol/admin/parse.h>

#include <protocol/admin/op.h>
#include <protocol/admin/reply.h>

#include <buffer/cc_buf.h>
#include <cc_debug.h>

#include <ctype.h>

static inline void
_skip_whitespace(struct buf *buf)
{
    for (; isspace(*buf->rpos) && buf->rpos < buf->wpos; ++buf->rpos);
}

static inline parse_rstatus_t
_get_op_type(struct op *op, struct bstring *type)
{
    ASSERT(op->type == OP_UNKNOWN);

    switch (type->len) {
    case 4:
        if (str4cmp(type->data, 'q', 'u', 'i', 't')) {
            op->type = OP_QUIT;
            break;
        }
    case 5:
        if (str5cmp(type->data, 's', 't', 'a', 't', 's')) {
            op->type = OP_STATS;
            break;
        }

        if (str5cmp(type->data, 'f', 'l', 'u', 's', 'h')) {
            op->type = OP_FLUSH;
            break;
        }
    case 7:
        if (str7cmp(type->data, 'v', 'e', 'r', 's', 'i', 'o', 'n')) {
            op->type = OP_VERSION;
            break;
        }
    }

    if (op->type == OP_UNKNOWN) { /* no match */
        log_warn("ill formatted request: unknown command");
        return PARSE_EINVALID;
    }

    return PARSE_OK;
}

parse_rstatus_t
parse_op(struct op *op, struct buf *buf)
{
    char *p;
    struct bstring type;
    parse_rstatus_t status;

    _skip_whitespace(buf);

    for (p = buf->rpos; p < buf->wpos; ++p) {
        if (isspace(*p)) {
            type.data = buf->rpos;
            type.len = p - buf->rpos;
            ASSERT(type.len > 0);

            status = _get_op_type(op, &type);

            buf->rpos = p + 1;
            ASSERT(buf->rpos <= buf->wpos);

            op->state = OP_PARSED;

            return status;
        }
    }

    return PARSE_EUNFIN;
}
