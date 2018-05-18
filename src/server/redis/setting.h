#pragma once

#include "data/process.h"

#include "core/core.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"
#include "protocol/data/redis_include.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related */
/*          name            type                default description */
#define REDIS_OPTION(ACTION)                                                            \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,  "daemonize the process"        )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,   "file storing the pid"         )\
    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,    "debug log flush interval(ms)" )

typedef struct {
    REDIS_OPTION(OPTION_DECLARE)
} redis_options_st;

struct setting {
    /* top-level */
    redis_options_st        redis;
    /* application modules */
    admin_options_st        admin;
    server_options_st       server;
    worker_options_st       worker;
    process_options_st      process;
    request_options_st      request;
    response_options_st     response;
    slab_options_st         slab;
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
