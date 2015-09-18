#pragma once

#include <cc_debug.h>
#include <cc_option.h>

/* option related */
/*          name            type                default         description */
#define SERVER_OPTION(ACTION)                                                               \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   "no",           "daemonize the process"    )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,           "file storing the pid"     )\
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "63790",        "port listening on"        )

#define SETTING(ACTION)         \
    DEBUG_OPTION(ACTION)        \
    SERVER_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};
