#pragma once


/*          type                string          # of args */
#define REQ_HASH(ACTION)                                \
    ACTION( REQ_HDEL,           "hdel",         3,  -1 )\
    ACTION( REQ_HDELALL,        "hdelall",      2,   0 )\
    ACTION( REQ_HEXISTS,        "hexists",      3,   0 )\
    ACTION( REQ_HGET,           "hget",         3,   0 )\
    ACTION( REQ_HGETALL,        "hgetall",      2,   0 )\
    ACTION( REQ_HINCRBY,        "hincrby",      4,   0 )\
    ACTION( REQ_HINCRBYFLOAT,   "hincrbyfloat", 4,   0 )\
    ACTION( REQ_HKEYS,          "hkeys",        2,   0 )\
    ACTION( REQ_HLEN,           "hlen",         2,   0 )\
    ACTION( REQ_HMGET,          "hmget",        3,  -1 )\
    ACTION( REQ_HMSET,          "hmset",        4,  -1 )\
    ACTION( REQ_HSET,           "hset",         4,   0 )\
    ACTION( REQ_HSETNX,         "hsetnx",       4,   0 )\
    ACTION( REQ_HSTRLEN,        "hstrlen",      3,   0 )\
    ACTION( REQ_HVALS,          "hvals",        2,   0 )\
    ACTION( REQ_HSCAN,          "hscan",        3,   0 )

/* "hlen KEY" == "*2\r\n$4\r\nhlen\r\n$3\r\nKEY\r\n" */
