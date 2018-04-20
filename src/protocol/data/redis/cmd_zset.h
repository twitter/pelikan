#pragma once

/*          type                    string              # of args */
#define REQ_ZSET(ACTION)                                        \
    ACTION( REQ_ZADD,               "zadd",             4,  -1 )\
    ACTION( REQ_ZINCRBY,            "zincrby",          4,  0  )\
    ACTION( REQ_ZREM,               "zrem",             3,  -1 )\
    ACTION( REQ_ZREMRANGEBYSCORE,   "zremrangebyscore", 4,  0  )\
    ACTION( REQ_ZREMRANGEBYRANK,    "zremrangebyrank",  4,  0  )\
    ACTION( REQ_ZREMRANGEBYLEX,     "zremrangebylex",   4,  0  )\
    ACTION( REQ_ZUNIONSTORE,        "zunionstore",      4,  -1 )\
    ACTION( REQ_ZINTERSTORE,        "zinterstore",      4,  -1 )\
    ACTION( REQ_ZRANGE,             "zrange",           4,  -1 )\
    ACTION( REQ_ZRANGEBYSCORE,      "zrangebyscore",    4,  -1 )\
    ACTION( REQ_ZREVRANGEBYSCORE,   "zrevrangebyscore", 4,  -1 )\
    ACTION( REQ_ZRANGEBYLEX,        "zrangebylex",      4,  -1 )\
    ACTION( REQ_ZREVRANGEBYLEX,     "zrevrangebylex",   4,  -1 )\
    ACTION( REQ_ZCOUNT,             "zcount",           4,  0  )\
    ACTION( REQ_ZLEXCOUNT,          "zlexcount",        4,  0  )\
    ACTION( REQ_ZREVRANGE,          "zrevrange",        4,  -1 )\
    ACTION( REQ_ZCARD,              "zcard",            2,  0  )\
    ACTION( REQ_ZSCORE,             "zscore",           3,  0  )\
    ACTION( REQ_ZRANK,              "zrank",            3,  0  )\
    ACTION( REQ_ZREVRANK,           "zrevrank",         3,  0  )\
    ACTION( REQ_ZSCAN,              "zscan",            3,  -1 )
