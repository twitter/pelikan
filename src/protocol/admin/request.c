#include <protocol/admin/request.h>

#include <cc_debug.h>
#include <cc_mm.h>

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring req_strings[] = {
    REQ_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

struct request *
request_create(void)
{
    struct request *req = cc_alloc(sizeof(struct request));

    if (req == NULL) {
        return NULL;
    }

    request_reset(req);

    return req;
}

void
request_destroy(struct request **req)
{
    ASSERT(*req != NULL);

    cc_free(*req);
    *req = NULL;
}

void
request_reset(struct request *req)
{
    ASSERT(req != NULL);

    req->state = REQ_PARSING;
    req->type = REQ_UNKNOWN;
    bstring_init(&req->arg);
}
