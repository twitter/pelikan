#pragma once

#include "data/process.h"

#include "core/core.h"
#include "protocol/data/memcache_include.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"
#include "time/time.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_option.h>
#include <cc_ring_array.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/* option related                                                                                            */
/*          name            type                default     description                                      */
#define CDB_OPTION(ACTION)                                                                                    \
    ACTION( daemonize,      OPTION_TYPE_BOOL,   false,      "daemonize the process"                          )\
    ACTION( pid_filename,   OPTION_TYPE_STR,    NULL,       "file storing the pid"                           )\
    ACTION( cdb_file_path,  OPTION_TYPE_STR,    "db.cdb",   "location of the .cdb file"                      )\
    ACTION( use_mmap,       OPTION_TYPE_BOOL,   false,      "use mmap to load the file, false: use the heap" )\
    ACTION( dlog_intvl,     OPTION_TYPE_UINT,   500,        "debug log flush interval(ms)"                   )\
    ACTION( klog_intvl,     OPTION_TYPE_UINT,   100,        "cmd log flush interval(ms)"                     )

typedef struct {
    CDB_OPTION(OPTION_DECLARE)
} cdb_options_st;

struct setting {
    /* top-level */
    cdb_options_st          cdb;
    /* application modules */
    admin_options_st        admin;
    server_options_st       server;
    worker_options_st       worker;
    process_options_st      process;
    klog_options_st         klog;
    request_options_st      request;
    response_options_st     response;
    time_options_st         time;
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
