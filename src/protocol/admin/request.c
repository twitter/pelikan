#include "request.h"

#include <cc_debug.h>
#include <cc_mm.h>

struct request *
admin_request_create(void)
{
    struct request *req = cc_alloc(sizeof(struct request));

    if (req == NULL) {
        return NULL;
    }

    admin_request_reset(req);

    return req;
}

void
admin_request_destroy(struct request **req)
{
    ASSERT(*req != NULL);

    cc_free(*req);
    *req = NULL;
}

void
admin_request_reset(struct request *req)
{
    ASSERT(req != NULL);

    req->state = REQ_PARSING;
    req->type = REQ_UNKNOWN;
    bstring_init(&req->arg);
}
