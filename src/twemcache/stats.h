#pragma once

#include <protocol/data/memcache_include.h>
#include <storage/slab/item.h>
#include <storage/slab/slab.h>
#include <twemcache/admin/process.h>
#include <twemcache/data/process.h>
#include <core/core.h>
#include <util/procinfo.h>
#include <util/stats.h>

#include <cc_event.h>
#include <cc_log.h>
#include <channel/cc_tcp.h>
#include <time/cc_wheel.h>

struct glob_stats {
    buf_metrics_st              buf_metrics;
    compose_rsp_metrics_st      compose_rsp_metrics;
    event_metrics_st            event_metrics;
    item_metrics_st             item_metrics;
    log_metrics_st              log_metrics;
    klog_metrics_st             klog_metrics;
    parse_req_metrics_st        parse_req_metrics;
    process_metrics_st          process_metrics;
    admin_process_metrics_st    admin_process_metrics;
    procinfo_metrics_st         procinfo_metrics;
    request_metrics_st          request_metrics;
    response_metrics_st         response_metrics;
    server_metrics_st           server_metrics;
    slab_metrics_st             slab_metrics;
    tcp_metrics_st              tcp_metrics;
    timing_wheel_metrics_st     timing_wheel_metrics;
    worker_metrics_st           worker_metrics;
};

extern struct glob_stats glob_stats;
