#include <protocol/admin/parse.h>

#include <protocol/admin/op.h>
#include <protocol/admin/reply.h>

#include <buffer/cc_buf.h>
#include <cc_debug.h>

#include <ctype.h>

static inline bool
_is_crlf(struct buf *buf, char *p)
{
    if (*p != CR || buf->wpos == p + 1) {
        return false;
    }

    if (*(p + 1) == LF) {
        return true;
    }

    return false;
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

        break;

    case 5:
        if (str5cmp(type->data, 's', 't', 'a', 't', 's')) {
            op->type = OP_STATS;
            break;
        }

        break;

    case 7:
        if (str7cmp(type->data, 'v', 'e', 'r', 's', 'i', 'o', 'n')) {
            op->type = OP_VERSION;
            break;
        }

        break;
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
    char *p, *q;
    struct bstring type;

    while (*buf->rpos == ' ' && buf->rpos < buf->wpos) {
        buf->rpos++;
    }
    p = q = buf->rpos;

    /* First find CRLF, this simplifies parsing. For admin port we don't care
     * much about efficiency.
     */
    for (; !_is_crlf(buf, p) && p < buf->wpos; p++);
    if (p == buf->wpos) {
        return PARSE_EUNFIN;
    }

    for (; *q != ' ' && q < p; q++);

    type.data = buf->rpos;
    type.len = q - buf->rpos;
    if (p < q) { /* intentional: pointing to the leading space */
        op->arg.len = p - q;
        op->arg.data = q;
    }
    op->state = OP_PARSED;
    buf->rpos = p + CRLF_LEN;
    return _get_op_type(op, &type);
}
