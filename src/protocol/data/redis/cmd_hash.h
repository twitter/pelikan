#pragma once

/*
 * Note: negative # of arguments means variable number of arguments:
 *   e.g. `-2' means at least two arguments. This notation is inherited from
 *   the original Redis server implementation.
 */

/*          type                string          # of args */
#define REQ_HASH(ACTION)                            \
    ACTION( REQ_HDEL,           "hdel",         -3 )\
    ACTION( REQ_HDELALL,        "hdelall",      2  )\
    ACTION( REQ_HEXISTS,        "hexists",      3  )\
    ACTION( REQ_HGET,           "hget",         3  )\
    ACTION( REQ_HGETALL,        "hgetall",      2  )\
    ACTION( REQ_HINCRBY,        "hincrby",      4  )\
    ACTION( REQ_HINCRBYFLOAT,   "hincrbyfloat", 4  )\
    ACTION( REQ_HKEYS,          "hkeys",        2  )\
    ACTION( REQ_HLEN,           "hlen",         2  )\
    ACTION( REQ_HMGET,          "hmget",        -3 )\
    ACTION( REQ_HMSET,          "hmset",        -4 )\
    ACTION( REQ_HSET,           "hset",         4  )\
    ACTION( REQ_HSETNX,         "hsetnx",       4  )\
    ACTION( REQ_HSTRLEN,        "hstrlen",      3  )\
    ACTION( REQ_HVALS,          "hvals",        2  )\
    ACTION( REQ_HSCAN,          "hscan",        -3 )

/* "hlen KEY" == "*2\r\n$4\r\nhlen\r\n$3\r\nKEY\r\n" */
