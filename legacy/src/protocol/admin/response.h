#pragma once

#include <cc_bstring.h>
#include <cc_queue.h>

#define RSP_TYPE_MSG(ACTION)                   \
    ACTION( RSP_GENERIC,      ""              )\
    ACTION( RSP_OK,           "OK\r\n"        )\
    ACTION( RSP_INVALID,      "INVALID\r\n"   )

#define GET_TYPE(_name, _str) _name,
typedef enum response_type {
    RSP_TYPE_MSG(GET_TYPE)
    RSP_SENTINEL
} response_type_t;
#undef GET_TYPE

struct response {
    response_type_t type;
    struct bstring  data;    /* value string */
};

struct response *admin_response_create(void);
void admin_response_destroy(struct response **rsp);
void admin_response_reset(struct response *rsp);
