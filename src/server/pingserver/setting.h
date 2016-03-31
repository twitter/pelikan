#pragma once

#include "admin/process.h"
#include "data/process.h"

#include <core/core.h>
#include <protocol/data/ping_include.h>

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_array.h>
#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <cc_signal.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related */
/*          name            type                default description */
#define PINGSERVER_OPTION(ACTION)                                                        \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,  "daemonize the process"        )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,   "file storing the pid"         )\
    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,    "debug log flush interval(ms)" )

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
    /* ccommon libraries */
    array_options_st        array;
    buf_options_st          buf;
    dbuf_options_st         dbuf;
    debug_options_st        debug;
    sockio_options_st       sockio;
    tcp_options_st          tcp;
};

extern struct setting setting;
extern unsigned int nopt;
