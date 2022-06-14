#include "time/time.h"

#include <cc_debug.h>
#include <time/cc_timer.h>

#include <sysexits.h>

time_t time_start;
proc_time_i proc_sec;
proc_time_fine_i proc_ms;
proc_time_fine_i proc_us;
proc_time_fine_i proc_ns;

static struct duration start;
static struct duration proc_snapshot;

uint8_t time_type = TIME_UNIX;

void
time_update(void)
{
    duration_snapshot(&proc_snapshot, &start);

    __atomic_store_n(&proc_sec, (proc_time_i)duration_sec(&proc_snapshot),
            __ATOMIC_RELAXED);
    __atomic_store_n(&proc_ms, (proc_time_fine_i)duration_ms(&proc_snapshot),
            __ATOMIC_RELAXED);
    __atomic_store_n(&proc_us, (proc_time_fine_i)duration_us(&proc_snapshot),
            __ATOMIC_RELAXED);
    __atomic_store_n(&proc_ns, (proc_time_fine_i)duration_ns(&proc_snapshot),
            __ATOMIC_RELAXED);
}

void
time_setup(time_options_st *options)
{
    if (options != NULL) {
        time_type = option_uint(&options->time_type);
    }

    time_start = time(NULL);
    duration_start(&start);
    time_update();

    log_info("timer started at %"PRIu64, (uint64_t)time_start);

    if (time_type >= TIME_SENTINEL) {
        exit(EX_CONFIG);
    }
}

void
time_teardown(void)
{
    duration_reset(&start);
    duration_reset(&proc_snapshot);

    log_info("timer ended at %"PRIu64, (uint64_t)time(NULL));
}
