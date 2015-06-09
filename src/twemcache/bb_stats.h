#ifndef _BB_TSTATS_H_
#define _BB_TSTATS_H_

/* (kyang) Very minimal stats for now, only included so the codec/request modules
   link properly. Full stats will be implemented later. */

#include <protocol/memcache/bb_codec.h>
#include <protocol/memcache/bb_request.h>
#include <util/bb_core.h>
#include <util/bb_procinfo.h>

#include <cc_event.h>
#include <channel/cc_tcp.h>

struct glob_stats {
    procinfo_metrics_st procinfo_metrics;
    event_metrics_st    event_metrics;
    server_metrics_st   server_metrics;
    worker_metrics_st   worker_metrics;
    tcp_metrics_st      tcp_metrics;
    codec_metrics_st    codec_metrics;
    request_metrics_st  request_metrics;
};

struct glob_stats glob_stats;

#endif /* _BB_TSTATS_H_ */
