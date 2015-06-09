#pragma once

#include <storage/slab/bb_slab.h>
#include <protocol/memcache/bb_request.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_log.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related */
/*          name            type                default         description */
#define SERVER_OPTION(ACTION)                                                               \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   "no",           "daemonize the process"    )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,           "file storing the pid"     )\
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "12321",        "port listening on"        )

#define SETTING(ACTION)       \
    SLAB_OPTION(ACTION)       \
    REQUEST_OPTION(ACTION)    \
    BUF_OPTION(ACTION)        \
    DBUF_OPTION(ACTION)       \
    LOG_OPTION(ACTION)        \
    TCP_OPTION(ACTION)        \
    SOCKIO_OPTION(ACTION)     \
    RING_ARRAY_OPTION(ACTION) \
    SERVER_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};
