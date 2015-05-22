#ifndef _BB_SETTING_H_
#define _BB_SETTING_H_

#include <storage/cuckoo/bb_cuckoo.h>
#include <protocol/memcache/bb_request.h>

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_log.h>
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
    ACTION( daemonize,      OPTION_TYPE_BOOL,   "no",           "daemonize the process"    )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,           "file storing the pid"     )\
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "22222",        "port listening on"        )

/* we compose our setting by including options needed by modules we use */
#define SETTING(ACTION)             \
    ARRAY_OPTION(ACTION)            \
    SOCKIO_OPTION(ACTION)           \
    CUCKOO_OPTION(ACTION)           \
    LOG_OPTION(ACTION)              \
    BUF_OPTION(ACTION)              \
    TCP_OPTION(ACTION)              \
    RING_ARRAY_OPTION(ACTION)       \
    REQUEST_OPTION(ACTION)          \
    SERVER_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};

#endif /* _BB_SETTING_H_ */
