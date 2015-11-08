#pragma once

#include <cc_bstring.h>

#define OP_TYPE_MSG(ACTION)                  \
    ACTION( OP_UNKNOWN,       ""            )\
    ACTION( OP_STATS,         "stats"       )\
    ACTION( OP_FLUSH,         "flush"       )\
    ACTION( OP_VERSION,       "version"     )\
    ACTION( OP_QUIT,          "quit"        )

#define GET_TYPE(_name, _str) _name,
typedef enum op_type {
    OP_TYPE_MSG(GET_TYPE)
    OP_SENTINEL
} op_type_t;
#undef GET_TYPE

extern struct bstring op_strings[OP_SENTINEL];

typedef enum op_state {
    OP_PARSING,
    OP_PARSED,
    OP_PROCESSING,
    OP_DONE,
} op_state_t;

struct op {
    op_state_t         state; /* op state */
    op_type_t          type;
};

struct op *op_create(void);
void op_destroy(struct op **op);
void op_reset(struct op *op);
