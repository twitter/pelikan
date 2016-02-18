#pragma once

#include <cc_bstring.h>
#include <cc_queue.h>

#define REP_TYPE_MSG(ACTION)                   \
    ACTION( REP_GENERIC,      ""              )\
    ACTION( REP_OK,           "OK\r\n"        )\
    ACTION( REP_INVALID,      "INVALID\r\n"   )

#define GET_TYPE(_name, _str) _name,
typedef enum reply_type {
    REP_TYPE_MSG(GET_TYPE)
    REP_SENTINEL
} reply_type_t;
#undef GET_TYPE

extern struct bstring reply_strings[REP_SENTINEL];

struct reply {
    reply_type_t         type;
    struct bstring       data;    /* value string */
};

struct reply *reply_create(void);
void reply_destroy(struct reply **rep);
void reply_reset(struct reply *rep);
