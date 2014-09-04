#ifndef __BB_RESPONSE_H__
#define __BB_RESPONSE_H__

#include <cc_bstring.h>
#include <cc_define.h>

/* TODO(yao): move this into the header */
#define RSP_TABLE(ACTION)                           \
    ACTION( RSP_VALUE,          "VALUE "          ) \
    ACTION( RSP_STAT,           "STAT "           ) \
    ACTION( RSP_END,            "END\r\n"         ) \
    ACTION( RSP_STORED,         "STORED\r\n"      ) \
    ACTION( RSP_NOT_STORED,     "NOT_STORED\r\n"  ) \
    ACTION( RSP_EXISTS,         "EXISTS\r\n"      ) \
    ACTION( RSP_NOT_FOUND,      "NOT_FOUND\r\n"   ) \
    ACTION( RSP_DELETED,        "DELETED\r\n"     ) \
    ACTION( RSP_CLIENT_ERROR,   "CLIENT_ERROR "   ) \
    ACTION( RSP_SERVER_ERROR,   "SERVER_ERROR "   ) \

#define GET_INDEX(_name, _str) _name,
typedef enum rsp_index {
    RSP_TABLE( GET_INDEX )
    RSP_SENTINAL
} rsp_index_t;
#undef GET_INDEX

#define GET_STRING(_name, _str) str2bstr(_str),
struct bstring rsp_strings[] = {
    RSP_TABLE( GET_STRING )
    null_bstring
};
#undef GET_STRING

#endif
