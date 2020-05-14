#pragma once

#include "admin/process.h"
#include "data/process.h"

#include "core/core.h"
#include "hotkey/hotkey.h"
#include "storage/cuckoo/cuckoo.h"
#include "protocol/data/memcache_include.h"
#include "time/time.h"

#include <buffer/cc_buf.h>
#include <cc_array.h>
#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <cc_signal.h>
#include <cc_stats_log.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related */
/*          name            type                default description */
#define SLIMCACHE_OPTION(ACTION)                                                        \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,  "daemonize the process"        )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,   "file storing the pid"         )\
    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,    "debug log flush interval(ms)" )\
    ACTION( klog_intvl,     OPTION_TYPE_UINT,   100,    "cmd log flush interval(ms)"   )\
    ACTION( stats_intvl,    OPTION_TYPE_UINT,   100,    "stats dump interval(ms)"      )

typedef struct {
    SLIMCACHE_OPTION(OPTION_DECLARE)
} slimcache_options_st;

struct setting {
    /* top-level */
    slimcache_options_st    slimcache;
    /* application modules */
    admin_options_st        admin;
    server_options_st       server;
    worker_options_st       worker;
    process_options_st      process;
    klog_options_st         klog;
    hotkey_options_st       hotkey;
    request_options_st      request;
    response_options_st     response;
    cuckoo_options_st       cuckoo;
    time_options_st         time;
    /* ccommon libraries */
    array_options_st        array;
    buf_options_st          buf;
    dbuf_options_st         dbuf;
    debug_options_st        debug;
    sockio_options_st       sockio;
    stats_log_options_st    stats_log;
    tcp_options_st          tcp;
};

extern struct setting setting;
extern unsigned int nopt;
