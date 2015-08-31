#pragma once

#include <cc_debug.h>
#include <cc_option.h>
#include <channel/cc_tcp.h>

/*          name         type             default  description */
#define SERVER_OPTION(ACTION)                                                \
    ACTION( server_host, OPTION_TYPE_STR, NULL,    "interfaces querying on" )\
    ACTION( server_port, OPTION_TYPE_STR, "12321", "port querying on"       )

#define SETTING(ACTION)      \
    SERVER_OPTION(ACTION)    \
    LOG_DEBUG_OPTION(ACTION) \
    TCP_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};
