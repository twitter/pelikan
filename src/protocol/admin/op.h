#pragma once

#include <cc_bstring.h>

/**
 * The admin protocol is not well defined and has not been with most protocols
 * (memcached, redis) I have seen. So what is included here is only loosely
 * defined for now and subject to future change.
 *
 * In general, admin port/protocol are on the slow path and mostly used by
 * system operators and stats collectors. Therefore, it has much less exposure
 * than the data port, greater variety in functionalities and simpler syntax.
 *
 * Here we assume all admin commands start with a main "verb", which determines
 * the "type" of operation. It can provide argument(s), which are optional. For
 * example, memcached uses "stats slab" to query per-slab metrics, and redis
 * allows querying different sections of the stats by calling "info <section>".
 * We are not particularly interested in parsing these commands in the parser,
 * since different binaries tend to have different interpretations of them, so
 * the argument(s) are not further parsed, but provided to the processor as a
 * whole blob.
 */

#define OP_TYPE_MSG(ACTION)                  \
    ACTION( OP_UNKNOWN,       ""            )\
    ACTION( OP_STATS,         "stats"       )\
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
    op_state_t      state; /* op state */
    op_type_t       type;
    struct bstring  arg;
};

struct op *op_create(void);
void op_destroy(struct op **op);
void op_reset(struct op *op);
