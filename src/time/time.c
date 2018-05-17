#include "time.h"

#include <cc_debug.h>
#include <cc_event.h>

#include <errno.h>
#include <stdbool.h>
#include <string.h>
#include <sys/time.h>

time_t time_start;
rel_time_t now;
rel_time_t now_usec;

void
time_update(void)
{
    int status;

#if defined OS_LINUX
    struct timespec timer;
    status = clock_gettime(CLOCK_REALTIME_COARSE, &timer);
    now_usec = (rel_time_t)(timer.tv_nsec / 1000);
#else
    struct timeval timer;
    status = gettimeofday(&timer, NULL);
    now_usec = (rel_time_t)(timer.tv_usec);
#endif

    /* we assume service is online for less than 2^32 seconds */
    now = (rel_time_t)(timer.tv_sec - time_start);

    if (status < 0) {
	log_warn("get current time failed: %s", strerror(errno));
    }

    log_vverb("internal timer updated to %u", now);
}

void
time_setup(void)
{
    /*
     * Make the time we started always be 2 seconds before we really
     * did, so time_now(0) - time.started is never zero. If so, things
     * like 'settings.oldest_live' which act as booleans as well as
     * values are now false in boolean context.
     */
    time_start = time(NULL) - 2;

    log_info("timer started at %"PRIu64"(2 sec setback)",
            (uint64_t)time_start);
}

void
time_teardown(void)
{
    log_info("timer ended at %"PRIu64, (uint64_t)time(NULL));
}
