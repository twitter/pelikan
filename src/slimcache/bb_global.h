#ifndef _BB_GLOBAL_H_
#define _BB_GLOBAL_H_

#include <cuckoo/bb_cuckoo.h>
#include <memcache/bb_request.h>

#include <cc_array.h>
#include <cc_define.h>
#include <cc_log.h>
#include <cc_mbuf.h>
#include <cc_nio.h>
#include <cc_option.h>
#include <cc_stats.h>
#include <cc_stream.h>


/* option related */
/*          name            type                default     description */
#define SERVER_OPTION(ACTION)                                                           \
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,       "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "22222",    "port listening on"        )

/* we compose our setting by including options needed by modules we use */
#define SETTING(ACTION)             \
    ARRAY_OPTION(ACTION)            \
    CUCKOO_OPTION(ACTION)           \
    ITEM_OPTION(ACTION)             \
    LOG_OPTION(ACTION)              \
    MBUF_OPTION(ACTION)             \
    NIO_OPTION(ACTION)              \
    REQUEST_OPTION(ACTION)          \
    SERVER_OPTION(ACTION)           \
    STREAM_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};

/* stats related */
/* stime, utime and maxrss are repeated/renamed for backward compatibility */
/*          name            type            description */
#define SYSTEM_METRIC(ACTION)                                           \
    ACTION( pid,            METRIC_DINTMAX, "pid of current process"   )\
    ACTION( time,           METRIC_DINTMAX, "unix time in seconds"     )\
    ACTION( uptime,         METRIC_DINTMAX, "process uptime in seconds")\
    ACTION( version,        METRIC_DINTMAX, "version as an int"        )\
    ACTION( rusage_system,  METRIC_DDOUBLE, "system CPU time"          )\
    ACTION( rusage_user,    METRIC_DDOUBLE, "user CPU time"            )\
    ACTION( rusage_maxrss,  METRIC_DINTMAX, "max RSS size"             )\
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
    ITEM_METRIC(ACTION)             \
    SYSTEM_METRIC(ACTION)

struct glob_stats {
    STATS(STATS_DECLARE)
};

#endif /* _BB_GLOBAL_H_ */
