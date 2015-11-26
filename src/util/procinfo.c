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

    UPDATE_VAL(procinfo_metrics, pid, getpid());
    UPDATE_VAL(procinfo_metrics, time, time_now_abs());
    UPDATE_VAL(procinfo_metrics, uptime, time_now());

    /* "%02d%02d%02d" % (major, minor, patch) */
    UPDATE_VAL(procinfo_metrics, version, VERSION_MAJOR * 10000 +
            VERSION_MINOR * 100 + VERSION_PATCH);

    /* not checking return as both parameters should be valid */
    getrusage(RUSAGE_SELF, &usage);

    UPDATE_VAL(procinfo_metrics,    ru_utime,       usage.ru_utime.tv_sec +
            usage.ru_utime.tv_usec * USEC);
    UPDATE_VAL(procinfo_metrics,    ru_stime,       usage.ru_stime.tv_sec +
            usage.ru_stime.tv_usec * USEC);
    UPDATE_VAL(procinfo_metrics,    ru_maxrss,      usage.ru_maxrss  );
    UPDATE_VAL(procinfo_metrics,    ru_ixrss,       usage.ru_ixrss   );
    UPDATE_VAL(procinfo_metrics,    ru_idrss,       usage.ru_idrss   );
    UPDATE_VAL(procinfo_metrics,    ru_isrss,       usage.ru_isrss   );
    UPDATE_VAL(procinfo_metrics,    ru_minflt,      usage.ru_minflt  );
    UPDATE_VAL(procinfo_metrics,    ru_majflt,      usage.ru_majflt  );
    UPDATE_VAL(procinfo_metrics,    ru_nswap,       usage.ru_nswap   );
    UPDATE_VAL(procinfo_metrics,    ru_inblock,     usage.ru_inblock );
    UPDATE_VAL(procinfo_metrics,    ru_oublock,     usage.ru_oublock );
    UPDATE_VAL(procinfo_metrics,    ru_msgsnd,      usage.ru_msgsnd  );
    UPDATE_VAL(procinfo_metrics,    ru_msgrcv,      usage.ru_msgrcv  );
    UPDATE_VAL(procinfo_metrics,    ru_nsignals,    usage.ru_nsignals);
    UPDATE_VAL(procinfo_metrics,    ru_nvcsw,       usage.ru_nvcsw   );
    UPDATE_VAL(procinfo_metrics,    ru_nivcsw,      usage.ru_nivcsw  );
}
