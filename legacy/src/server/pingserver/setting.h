#pragma once

#include "admin/process.h"
#include "data/process.h"

#include "core/core.h"
#include "protocol/data/ping_include.h"
#include "time/time.h"

#include <buffer/cc_buf.h>
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
#define PINGSERVER_OPTION(ACTION)                                                        \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,  "daemonize the process"        )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,   "file storing the pid"         )\
    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,    "debug log flush interval(ms)" )\
    ACTION( stats_intvl,    OPTION_TYPE_UINT,   100,    "stats dump interval(ms)"      )

typedef struct {
    PINGSERVER_OPTION(OPTION_DECLARE)
} pingserver_options_st;

struct setting {
    /* top-level */
    pingserver_options_st   pingserver;
    /* application modules */
    admin_options_st        admin;
    server_options_st       server;
    worker_options_st       worker;
    time_options_st         time;
    /* ccommon libraries */
    buf_options_st          buf;
    debug_options_st        debug;
    sockio_options_st       sockio;
    stats_log_options_st    stats_log;
    tcp_options_st          tcp;
};

extern struct setting setting;
extern unsigned int nopt;
