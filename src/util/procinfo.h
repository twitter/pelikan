#pragma once

#include <cc_metric.h>

/* stime, utime and maxrss are repeated/renamed for backward compatibility */
/*          name            type            description */
#define PROCINFO_METRIC(ACTION)                                         \
    ACTION( pid,            METRIC_DINTMAX, "pid of current process"   )\
    ACTION( time,           METRIC_DINTMAX, "unix time in seconds"     )\
    ACTION( uptime,         METRIC_DINTMAX, "process uptime in seconds")\
    ACTION( version,        METRIC_DINTMAX, "version as an int"        )\
    ACTION( ru_stime,       METRIC_DDOUBLE, "system CPU time"          )\
    ACTION( ru_utime,       METRIC_DDOUBLE, "user CPU time"            )\
    ACTION( ru_maxrss,      METRIC_DINTMAX, "max RSS size"             )\
    ACTION( ru_ixrss,       METRIC_DINTMAX, "text memory size"         )\
    ACTION( ru_idrss,       METRIC_DINTMAX, "data memory size"         )\
    ACTION( ru_isrss,       METRIC_DINTMAX, "stack memory size"        )\
    ACTION( ru_minflt,      METRIC_DINTMAX, "pagefalut w/o I/O"        )\
    ACTION( ru_majflt,      METRIC_DINTMAX, "pagefalut w/ I/O"         )\
    ACTION( ru_nswap,       METRIC_DINTMAX, "# times swapped"          )\
    ACTION( ru_inblock,     METRIC_DINTMAX, "real FS input"            )\
    ACTION( ru_oublock,     METRIC_DINTMAX, "real FS output"           )\
    ACTION( ru_msgsnd,      METRIC_DINTMAX, "# IPC messages sent"      )\
    ACTION( ru_msgrcv,      METRIC_DINTMAX, "# IPC messages received"  )\
    ACTION( ru_nsignals,    METRIC_DINTMAX, "# signals delivered"      )\
    ACTION( ru_nvcsw,       METRIC_DINTMAX, "# voluntary CS"           )\
    ACTION( ru_nivcsw,      METRIC_DINTMAX, "# involuntary CS"         )

typedef struct {
    PROCINFO_METRIC(METRIC_DECLARE)
} procinfo_metrics_st;

#define PROCINFO_METRIC_INIT(_metrics) do {                                 \
    *(_metrics) = (procinfo_metrics_st) { PROCINFO_METRIC(METRIC_INIT) };   \
} while(0)

void procinfo_setup(procinfo_metrics_st *procinfo_metrics);
void procinfo_teardown(void);

void procinfo_update(void);
