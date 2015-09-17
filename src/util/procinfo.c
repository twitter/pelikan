#include <util/procinfo.h>

#include <time/time.h>

#include <cc_debug.h>

#include <stdbool.h>
#include <sys/resource.h>
#include <unistd.h>


#define PROCINFO_MODULE_NAME "util::procinfo"

static bool procinfo_init = false;
static procinfo_metrics_st *procinfo_metrics = NULL;

#define USEC 0.000001

void
procinfo_setup(procinfo_metrics_st *metrics)
{
    log_info("set up the %s module", PROCINFO_MODULE_NAME);

    procinfo_metrics = metrics;
    if (metrics != NULL) {
        PROCINFO_METRIC_INIT(procinfo_metrics);
    }

    if (procinfo_init) {
        log_warn("%s has already been setup, overwrite", PROCINFO_MODULE_NAME);
    }
    procinfo_init = true;
}

void
procinfo_teardown(void)
{
    log_info("tear down the %s module", PROCINFO_MODULE_NAME);

    if (!procinfo_init) {
        log_warn("%s has never been setup", PROCINFO_MODULE_NAME);
    }
    procinfo_metrics = NULL;
    procinfo_init = false;
}

void
procinfo_update(void)
{
    struct rusage usage;

    procinfo_metrics->pid.vintmax         = (intmax_t)getpid();
    procinfo_metrics->time.vintmax        = (intmax_t)time_now_abs();
    procinfo_metrics->uptime.vintmax      = (intmax_t)time_now();
    /* "%02d%02d%02d" % (major, minor, patch) */
    procinfo_metrics->version.vintmax     = (intmax_t)VERSION_MAJOR * 10000 +
            VERSION_MINOR * 100 + VERSION_PATCH;

    /* not checking return as both parameters should be valid */
    getrusage(RUSAGE_SELF, &usage);

    procinfo_metrics->ru_utime.vdouble    = (double)usage.ru_utime.tv_sec +
            (double)usage.ru_utime.tv_usec * USEC;
    procinfo_metrics->ru_stime.vdouble    = (double)usage.ru_stime.tv_sec +
            (double)usage.ru_stime.tv_usec * USEC;
    procinfo_metrics->ru_maxrss.vintmax   = (intmax_t)usage.ru_maxrss;
    procinfo_metrics->ru_ixrss.vintmax    = (intmax_t)usage.ru_ixrss;
    procinfo_metrics->ru_idrss.vintmax    = (intmax_t)usage.ru_idrss;
    procinfo_metrics->ru_isrss.vintmax    = (intmax_t)usage.ru_isrss;
    procinfo_metrics->ru_minflt.vintmax   = (intmax_t)usage.ru_minflt;
    procinfo_metrics->ru_majflt.vintmax   = (intmax_t)usage.ru_majflt;
    procinfo_metrics->ru_nswap.vintmax    = (intmax_t)usage.ru_nswap;
    procinfo_metrics->ru_inblock.vintmax  = (intmax_t)usage.ru_inblock;
    procinfo_metrics->ru_oublock.vintmax  = (intmax_t)usage.ru_oublock;
    procinfo_metrics->ru_msgsnd.vintmax   = (intmax_t)usage.ru_msgsnd;
    procinfo_metrics->ru_msgrcv.vintmax   = (intmax_t)usage.ru_msgrcv;
    procinfo_metrics->ru_nsignals.vintmax = (intmax_t)usage.ru_nsignals;
    procinfo_metrics->ru_nvcsw.vintmax    = (intmax_t)usage.ru_nvcsw;
    procinfo_metrics->ru_nivcsw.vintmax   = (intmax_t)usage.ru_nivcsw;
}
