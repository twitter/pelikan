#include <protocol/admin/op.h>

#include <cc_debug.h>
#include <cc_mm.h>

#define GET_STRING(_name, _str) {sizeof(_str) - 1, (_str)},
struct bstring op_strings[] = {
    OP_TYPE_MSG(GET_STRING)
};
#undef GET_STRING

struct op *
op_create(void)
{
    struct op *op = cc_alloc(sizeof(struct op));

    if (op == NULL) {
        return NULL;
    }

    op_reset(op);

    return op;
}

void
op_destroy(struct op **op)
{
    ASSERT(*op != NULL);

    cc_free(*op);
    *op = NULL;
}

void
op_reset(struct op *op)
{
    ASSERT(op != NULL);

    op->state = OP_PARSING;
    op->type = OP_UNKNOWN;
}
