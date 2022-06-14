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

#define REQ_TYPE_MSG(ACTION)                \
    ACTION( REQ_UNKNOWN,       ""          )\
    ACTION( REQ_STATS,         "stats"     )\
    ACTION( REQ_VERSION,       "version"   )\
    ACTION( REQ_QUIT,          "quit"      )

#define GET_TYPE(_name, _str) _name,
typedef enum request_type {
    REQ_TYPE_MSG(GET_TYPE)
    REQ_SENTINEL
} request_type_t;
#undef GET_TYPE

typedef enum request_state {
    REQ_PARSING,
    REQ_PARSED,
    REQ_DONE,
} request_state_t;

struct request {
    request_state_t      state; /* request state */
    request_type_t       type;
    struct bstring  arg;
};

struct request *admin_request_create(void);
void admin_request_destroy(struct request **req);
void admin_request_reset(struct request *req);
