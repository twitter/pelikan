#pragma once

#include <cc_bstring.h>
#include <cc_debug.h>

#include <stdint.h>


/* return -1, 0, 1 for <, =, > */
#define COMPARE(_a, _b) (-((_a) <= (_b)) + ((_a) >= (_b)))

typedef enum blob_type {
    BLOB_TYPE_UNKNOWN=0,
    BLOB_TYPE_INT=1,
    BLOB_TYPE_STR=2,
    BLOB_TYPE_SENTINEL
} blob_type_t;

struct blob {
    blob_type_t type;
    union {
        struct bstring vstr;
        uint64_t vint;
    };
};

static inline int
blob_compare(const struct blob *first, const struct blob *second)
{
    size_t len;
    int ret;

    ASSERT(first->type > BLOB_TYPE_UNKNOWN && first->type < BLOB_TYPE_SENTINEL);
    ASSERT(second->type > BLOB_TYPE_UNKNOWN && second->type < BLOB_TYPE_SENTINEL);

    if (first->type != second->type) {
        return COMPARE(first->type, second->type);
    } else {
        if (first->type == BLOB_TYPE_INT) {
            return COMPARE(first->vint, second->vint);
        } else { /* str */
            len = MIN(first->vstr.len, second->vstr.len);
            ret = memcmp(first->vstr.data, second->vstr.data, len);
            if (ret == 0) {
                return COMPARE(first->vstr.len, second->vstr.len);
            }

            return ret;
        }
    }
}
