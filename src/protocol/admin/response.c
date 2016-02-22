#include <protocol/admin/response.h>

#include <cc_debug.h>
#include <cc_mm.h>

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring rsp_strings[] = {
    RSP_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

struct response *
response_create(void)
{
    struct response *rsp = cc_alloc(sizeof(struct response));

    if (rsp == NULL) {
        return NULL;
    }

    response_reset(rsp);
    return rsp;
}

void
response_destroy(struct response **rsp)
{
    ASSERT(*rsp != NULL);

    cc_free(*rsp);
    *rsp = NULL;
}

void
response_reset(struct response *rsp)
{
    ASSERT(rsp != NULL);

    rsp->type = RSP_GENERIC;
    bstring_init(&rsp->data);
}
