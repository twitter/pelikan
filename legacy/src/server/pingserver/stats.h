#pragma once

#include "data/process.h"

#include "core/core.h"
#include "protocol/data/ping_include.h"
#include "util/procinfo.h"

#include <buffer/cc_dbuf.h>
#include <cc_event.h>
#include <cc_log.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>
#include <time/cc_wheel.h>

struct stats {
    /* perf info */
    procinfo_metrics_st         procinfo;
    /* application modules */
    parse_req_metrics_st        parse_req;
    compose_rsp_metrics_st      compose_rsp;
    server_metrics_st           server;
    worker_metrics_st           worker;
    /* ccommon libraries */
    buf_metrics_st              buf;
    dbuf_metrics_st             dbuf;
    event_metrics_st            event;
    log_metrics_st              log;
    sockio_metrics_st           sockio;
    tcp_metrics_st              tcp;
    timing_wheel_metrics_st     timing_wheel;
};

extern struct stats stats;
extern unsigned int nmetric;
