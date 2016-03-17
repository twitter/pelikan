#pragma once

#include <cc_metric.h>

/* stime, utime and maxrss are repeated/renamed for backward compatibility */
/*          name            type            description */
#define PROCINFO_METRIC(ACTION)                                         \
    ACTION( pid,            METRIC_GAUGE,   "pid of current process"   )\
    ACTION( time,           METRIC_COUNTER, "unix time in seconds"     )\
    ACTION( uptime,         METRIC_COUNTER, "process uptime in seconds")\
    ACTION( version,        METRIC_COUNTER, "version as an int"        )\
    ACTION( ru_stime,       METRIC_FPN,     "system CPU time"          )\
    ACTION( ru_utime,       METRIC_FPN,     "user CPU time"            )\
    ACTION( ru_maxrss,      METRIC_GAUGE,   "max RSS size"             )\
    ACTION( ru_ixrss,       METRIC_GAUGE,   "text memory size"         )\
    ACTION( ru_idrss,       METRIC_GAUGE,   "data memory size"         )\
    ACTION( ru_isrss,       METRIC_GAUGE,   "stack memory size"        )\
    ACTION( ru_minflt,      METRIC_COUNTER, "pagefalut w/o I/O"        )\
    ACTION( ru_majflt,      METRIC_COUNTER, "pagefalut w/ I/O"         )\
    ACTION( ru_nswap,       METRIC_COUNTER, "# times swapped"          )\
    ACTION( ru_inblock,     METRIC_COUNTER, "real FS input"            )\
    ACTION( ru_oublock,     METRIC_COUNTER, "real FS output"           )\
    ACTION( ru_msgsnd,      METRIC_COUNTER, "# IPC messages sent"      )\
    ACTION( ru_msgrcv,      METRIC_COUNTER, "# IPC messages received"  )\
    ACTION( ru_nsignals,    METRIC_COUNTER, "# signals delivered"      )\
    ACTION( ru_nvcsw,       METRIC_COUNTER, "# voluntary CS"           )\
    ACTION( ru_nivcsw,      METRIC_COUNTER, "# involuntary CS"         )

typedef struct {
    PROCINFO_METRIC(METRIC_DECLARE)
} procinfo_metrics_st;

void procinfo_setup(procinfo_metrics_st *procinfo_metrics);
void procinfo_teardown(void);

void procinfo_update(void);
