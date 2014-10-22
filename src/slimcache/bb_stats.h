#ifndef _BB_SSTATS_H_
#define _BB_SSTATS_H_

#include <cuckoo/bb_cuckoo.h>
#include <cuckoo/bb_item.h>
#include <memcache/bb_codec.h>
#include <memcache/bb_request.h>
#include <slimcache/bb_core.h>
#include <slimcache/bb_process.h>

#include <cc_define.h>
#include <cc_metric.h>


/* stime, utime and maxrss are repeated/renamed for backward compatibility */
/*          name            type            description */
#define SYSTEM_METRIC(ACTION)                                           \
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


#define STATS(ACTION)               \
    SYSTEM_METRIC(ACTION)           \
    CODEC_METRIC(ACTION)            \
    CORE_METRIC(ACTION)             \
    CUCKOO_METRIC(ACTION)           \
    ITEM_METRIC(ACTION)             \
    PROCESS_METRIC(ACTION)          \
    REQPOOL_METRIC(ACTION)

struct stats {
    STATS(METRIC_DECLARE)
};

extern struct stats Stats;
extern const unsigned int Nmetric;

#define METRIC_BASE (void *)&Stats
#define METRIC_PTR(_c) (struct metric *)(METRIC_BASE + offsetof(struct stats, _c))
#define INCR_N(_c, _d) do {                     \
    metric_incr_n(METRIC_PTR(_c), _d);          \
} while(0)
#define INCR(_c) INCR_N(_c, 1)
#define DECR_N(_c, _d) do {                     \
    metric_decr_n(METRIC_PTR(_c), _d);          \
} while(0)
#define DECR(_c) DECR_N(_c, 1)


#endif /* _BB_SSTATS_H_ */
