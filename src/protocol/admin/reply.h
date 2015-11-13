#pragma once

#include <cc_bstring.h>

#define REP_TYPE_MSG(ACTION)                   \
    ACTION( REP_UNKNOWN,      ""              )\
    ACTION( REP_STAT,         "STAT "         )\
    ACTION( REP_VERSION,      "VERSION "      )\
    ACTION( REP_CLIENT_ERROR, "CLIENT_ERROR " )\
    ACTION( REP_SERVER_ERROR, "SERVER_ERROR " )

#define GET_TYPE(_name, _str) _name,
typedef enum reply_type {
    REP_TYPE_MSG(GET_TYPE)
    REP_SENTINEL
} reply_type_t;
#undef GET_TYPE

extern struct bstring reply_strings[REP_SENTINEL];

typedef enum reply_state {
    REP_PARSING,
    REP_PARSED,
    REP_PROCESSING,
    REP_DONE,
} reply_state_t;

struct reply {
    reply_state_t        state;  /* reply state */
    reply_type_t         type;

    struct metric        *met;    /* metric, for reporting stats */
    struct bstring       vstr;    /* value string */
};

struct reply *reply_create(void);
void reply_destroy(struct reply **rep);
void reply_reset(struct reply *rep);
