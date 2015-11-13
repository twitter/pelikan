#include <protocol/admin/reply.h>

#include <cc_debug.h>
#include <cc_mm.h>

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring reply_strings[] = {
    REP_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

struct reply *
reply_create(void)
{
    struct reply *rep = cc_alloc(sizeof(struct reply));

    if (rep == NULL) {
        return NULL;
    }

    reply_reset(rep);
    return rep;
}

void
reply_destroy(struct reply **rep)
{
    ASSERT(*rep != NULL);

    cc_free(*rep);
    *rep = NULL;
}

void
reply_reset(struct reply *rep)
{
    ASSERT(rep != NULL);

    rep->state = REP_PARSING;
    rep->type = REP_UNKNOWN;
    rep->met = NULL;
    bstring_init(&rep->vstr);
}
