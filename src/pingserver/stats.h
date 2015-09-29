#pragma once

#include <util/procinfo.h>

struct glob_stats {
    procinfo_metrics_st     procinfo_metrics;
    log_metrics_st          log_metrics;
};

struct glob_stats glob_stats;
