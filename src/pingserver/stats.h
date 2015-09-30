#pragma once

#include <protocol/ping_include.h>
#include <pingserver/process.h>

#include <util/procinfo.h>
#include <cc_log.h>

struct glob_stats {
    procinfo_metrics_st     procinfo_metrics;
    log_metrics_st          log_metrics;
    request_metrics_st      request_metrics;
    response_metrics_st     response_metrics;
    parse_req_metrics_st    parse_req_metrics;
    compose_rsp_metrics_st  compose_rsp_metrics;
    process_metrics_st      process_metrics;
};

struct glob_stats glob_stats;
