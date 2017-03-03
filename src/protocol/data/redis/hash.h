#pragma once

#include <cc_define.h>

/*
 * Note: negative # of arguments means variable number of arguments:
 *   e.g. `-2' means at least two arguments. This notation is inherited from
 *   the original Redis server implementation.
 */
/*          type                string          # of arguments */
#define REQ_HASH(ACTION)                            \
    ACTION( REQ_HDEL,           "hdel",         -2 )\
    ACTION( REQ_HEXISTS,        "hexists",      2  )\
    ACTION( REQ_HGET,           "hget",         2  )\
    ACTION( REQ_HGETALL,        "hgetall",      1  )\
    ACTION( REQ_HINCRBY,        "hincrby",      3  )\
    ACTION( REQ_HINCRBYFLOAT,   "hincrbyfloat", 3  )\
    ACTION( REQ_HKEYS,          "hkeys",        1  )\
    ACTION( REQ_HLEN,           "hlen",         1  )\
    ACTION( REQ_HMGET,          "hmget",        -2 )\
    ACTION( REQ_HMSET,          "hmset",        -3 )\
    ACTION( REQ_HSET,           "hset",         3  )\
    ACTION( REQ_HSETNX,         "hsetnx",       3  )\
    ACTION( REQ_HSTRLEN,        "hstrlen",      2  )\
    ACTION( REQ_HVALS,          "hvals",        1  )\
    ACTION( REQ_HSCAN,          "hscan",        -2 )

#define GET_TYPE(_name, _str, narg) _name,
typedef enum command_table {
    REQ_UNKNOWN
    REQ_HASH(GET_TYPE)
    REQ_SENTINEL
} request_hash_t;
#undef GET_TYPE

struct hash_command {
    request_hash_t  type;
    struct bstring  key;
};
