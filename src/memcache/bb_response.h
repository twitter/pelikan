#ifndef __BB_RESPONSE_H__
#define __BB_RESPONSE_H__

/*
 * NOTE(yao): the asymmetry between request and response source files (and the
 * lack of response.c) is a result of asymmetrical handling of requests and
 * responses at the backend. Should we write a client, we will end up with
 * composing functions of requests and parsing functions of responses only. If
 * we write a proxy, we will end up implementing all the functionalities.
 */

#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_mbuf.h>
#include <cc_util.h>

#define RSP_MSG(ACTION)                             \
    ACTION( RSP_CRLF,           CRLF              ) \
    ACTION( RSP_VALUE,          "VALUE "          ) \
    ACTION( RSP_STAT,           "STAT "           ) \
    ACTION( RSP_END,            "END\r\n"         ) \
    ACTION( RSP_STORED,         "STORED\r\n"      ) \
    ACTION( RSP_NOT_STORED,     "NOT_STORED\r\n"  ) \
    ACTION( RSP_EXISTS,         "EXISTS\r\n"      ) \
    ACTION( RSP_NOT_FOUND,      "NOT_FOUND\r\n"   ) \
    ACTION( RSP_DELETED,        "DELETED\r\n"     ) \
    ACTION( RSP_CLIENT_ERROR,   "CLIENT_ERROR\r\n") \
    ACTION( RSP_SERVER_ERROR,   "SERVER_ERROR\r\n") \

#define GET_INDEX(_name, _str) _name,
typedef enum rsp_index {
    RSP_MSG( GET_INDEX )
    RSP_SENTINAL
} rsp_index_t;
#undef GET_INDEX

#endif
