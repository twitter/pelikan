#pragma once

#include <protocol/memcache/codec.h>
#include <protocol/memcache/request.h>
#include <storage/slab/item.h>
#include <storage/slab/slab.h>
#include <twemcache/process.h>
#include <core/core.h>
#include <util/procinfo.h>

#include <cc_event.h>
#include <channel/cc_tcp.h>

struct glob_stats {
    procinfo_metrics_st procinfo_metrics;
    event_metrics_st    event_metrics;
    server_metrics_st   server_metrics;
    worker_metrics_st   worker_metrics;
    buf_metrics_st      buf_metrics;
    tcp_metrics_st      tcp_metrics;
    codec_metrics_st    codec_metrics;
    request_metrics_st  request_metrics;
    process_metrics_st  process_metrics;
    slab_metrics_st     slab_metrics;
    item_metrics_st     item_metrics;
};

struct glob_stats glob_stats;
