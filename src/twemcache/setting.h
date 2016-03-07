#pragma once

#include <twemcache/data/process.h>

#include <core/admin/admin.h>
#include <storage/slab/slab.h>
#include <storage/slab/item.h>
#include <protocol/data/memcache_include.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related */
/*          name            type                default         description */
#define SERVER_OPTION(ACTION)                                                               \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,          "daemonize the process"    )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,           "file storing the pid"     )\
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "12321",        "port listening on"        )

#define SETTING(ACTION)         \
    ADMIN_OPTION(ACTION)        \
    ARRAY_OPTION(ACTION)        \
    BUF_OPTION(ACTION)          \
    DBUF_OPTION(ACTION)         \
    DEBUG_OPTION(ACTION)        \
    ITEM_OPTION(ACTION)         \
    KLOG_OPTION(ACTION)         \
    RING_ARRAY_OPTION(ACTION)   \
    REQUEST_OPTION(ACTION)      \
    RESPONSE_OPTION(ACTION)     \
    PROCESS_OPTION(ACTION)      \
    SERVER_OPTION(ACTION)       \
    SLAB_OPTION(ACTION)         \
    SOCKIO_OPTION(ACTION)       \
    TCP_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};
