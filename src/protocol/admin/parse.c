#include "parse.h"

#include "request.h"

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
_get_req_type(struct request *req, struct bstring *type)
{
    ASSERT(req->type == REQ_UNKNOWN);

    switch (type->len) {
    case 4:
        if (str4cmp(type->data, 'q', 'u', 'i', 't')) {
            req->type = REQ_QUIT;
            break;
        }

        break;

    case 5:
        if (str5cmp(type->data, 's', 't', 'a', 't', 's')) {
            req->type = REQ_STATS;
            break;
        }

        break;

    case 7:
        if (str7cmp(type->data, 'v', 'e', 'r', 's', 'i', 'o', 'n')) {
            req->type = REQ_VERSION;
            break;
        }

        break;
    }

    if (req->type == REQ_UNKNOWN) { /* no match */
        log_warn("ill formatted request: unknown command");
        return PARSE_EINVALID;
    }

    return PARSE_OK;
}

parse_rstatus_t
admin_parse_req(struct request *req, struct buf *buf)
{
    char *p, *q;
    struct bstring type;

    while (*buf->rpos == ' ' && buf->rpos < buf->wpos) {
        buf->rpos++;
    }
    p = q = buf->rpos;

    /* First find CRLF and store it in p, this simplifies parsing.
     * For admin port we don't care much about efficiency.
     */
    for (; !_is_crlf(buf, p) && p < buf->wpos; p++);
    if (p == buf->wpos) {
        return PARSE_EUNFIN;
    }

    /* type: between rpos and q */
    for (; *q != ' ' && q < p; q++);
    type.data = buf->rpos;
    type.len = q - buf->rpos;

    if (p > q) { /* intentional: pointing to the leading space */
        req->arg.len = p - q;
        req->arg.data = q;
    }
    req->state = REQ_PARSED;
    buf->rpos = p + CRLF_LEN;
    return _get_req_type(req, &type);
}
