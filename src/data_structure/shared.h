#pragma once

#include <cc_bstring.h>

typedef enum blob_type {
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
