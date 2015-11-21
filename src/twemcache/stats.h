#pragma once

#include <protocol/memcache_include.h>
#include <storage/slab/item.h>
#include <storage/slab/slab.h>
#include <twemcache/process.h>
#include <core/core.h>
#include <util/procinfo.h>
#include <util/stats.h>

#include <cc_event.h>
#include <channel/cc_tcp.h>

struct glob_stats {
    procinfo_metrics_st     procinfo_metrics;
    event_metrics_st        event_metrics;
    server_metrics_st       server_metrics;
    worker_metrics_st       worker_metrics;
    buf_metrics_st          buf_metrics;
    tcp_metrics_st          tcp_metrics;
    request_metrics_st      request_metrics;
    response_metrics_st     response_metrics;
    parse_req_metrics_st    parse_req_metrics;
    compose_rsp_metrics_st  compose_rsp_metrics;
    process_metrics_st      process_metrics;
    slab_metrics_st         slab_metrics;
    item_metrics_st         item_metrics;
    log_metrics_st          log_metrics;
    klog_metrics_st         klog_metrics;
};

extern struct glob_stats glob_stats;
