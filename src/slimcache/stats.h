#pragma once

#include <protocol/data/memcache_include.h>
#include <slimcache/admin/process.h>
#include <slimcache/data/process.h>
#include <storage/cuckoo/cuckoo.h>
#include <core/core.h>
#include <util/procinfo.h>
#include <util/stats.h>

#include <cc_event.h>
#include <cc_log.h>
#include <channel/cc_tcp.h>
#include <time/cc_wheel.h>

struct stats {
    /* perf info */
    procinfo_metrics_st         procinfo;
    /* application modules */
    process_metrics_st          process;
    admin_process_metrics_st    admin_process;
    parse_req_metrics_st        parse_req;
    compose_rsp_metrics_st      compose_rsp;
    klog_metrics_st             klog;
    request_metrics_st          request;
    response_metrics_st         response;
    server_metrics_st           server;
    worker_metrics_st           worker;
    cuckoo_metrics_st           cuckoo;
    /* ccommon libraries */
    buf_metrics_st              buf;
    event_metrics_st            event;
    log_metrics_st              log;
    tcp_metrics_st              tcp;
    timing_wheel_metrics_st     timing_wheel;
};

extern struct stats stats;
