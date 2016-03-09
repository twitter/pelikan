#pragma once

#include <slimcache/admin/process.h>
#include <slimcache/data/process.h>

#include <core/core.h>
#include <storage/cuckoo/cuckoo.h>
#include <protocol/data/memcache_include.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <cc_signal.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

#define MAX_CONNS 1024          /* arbitrary number for now */

/* option related */
/*          name            type                default         description */
#define SERVER_OPTION(ACTION)                                                               \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,          "daemonize the process"    )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,           "file storing the pid"     )\
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "22222",        "port listening on"        )

/* we compose our setting by including options needed by modules we use */
#define SETTING(ACTION)         \
    ADMIN_OPTION(ACTION)        \
    ARRAY_OPTION(ACTION)        \
    BUF_OPTION(ACTION)          \
    CUCKOO_OPTION(ACTION)       \
    DBUF_OPTION(ACTION)         \
    DEBUG_OPTION(ACTION)        \
    KLOG_OPTION(ACTION)         \
    PROCESS_OPTION(ACTION)      \
    REQUEST_OPTION(ACTION)      \
    RESPONSE_OPTION(ACTION)     \
    RING_ARRAY_OPTION(ACTION)   \
    SERVER_OPTION(ACTION)       \
    SOCKIO_OPTION(ACTION)       \
    TCP_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};
