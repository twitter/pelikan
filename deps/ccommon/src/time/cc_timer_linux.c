#include <cc_timer.h>

#include <cc_debug.h>

#include <errno.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* Note(yao): linux/time64.h is not included in linux kernel before version 3.17
 * So we will have to define some macros for convresion, but we won't need them
 * if we use time64.h, which should happen before year 2038 :)
 */
#define NSEC_PER_USEC  1000L
#define NSEC_PER_MSEC  1000000L
#define NSEC_PER_SEC   1000000000L

/*
 * As described in this link:
 * http://nadeausoftware.com/articles/2012/04/c_c_tip_how_measure_elapsed_real_time_benchmarking
 * On linux we should choose CLOCK_MONOTONIC_RAW over CLOCK_MONOTONIC, as the
 * former aims to be more precise.
 * CLOCK_REALTIME can be reset when clock has drifted, so it may not always be
 * monotonic, and should be avoided if possible.
 */
#if defined(CLOCK_MONOTONIC_RAW)
static const clockid_t cid = CLOCK_MONOTONIC_RAW;
#elif defined(CLOCK_MONOTONIC)
static const clockid_t cid = CLOCK_MONOTONIC;
#elif defined(CLOCK_REALTIME)
static const clockid_t cid = CLOCK_REALTIME;
#else
static const clockid_t cid = (clockid_t)-1;
#endif

void
timer_reset(struct timer *t)
{
    ASSERT(t != NULL);

    t->started = false;
    t->stopped = false;
    t->start = 0;
    t->stop = 0;
}

static inline void
_timer_gettime(struct timespec *ts)
{
    int ret;

    ASSERT(cid != (clockid_t)-1);

    ret = clock_gettime(cid, ts);
    if (ret == -1) {
        /*
         * Note(yao): for the purpose of this module, it doesn't make much sense
         * to return an error even when the gettime call fails. So we may just
         * return an arbitrary value.
         * We still set t->started to true because we don't want to halt the
         * program due to a timer error, the purpose of these boolean fields is
         * to catch cases where people forget to start the time before stopping.
         */
        log_error("clock_gettime returns error, timer result undefined: %s",
                strerror(errno));
        ts->tv_sec = 0;
        ts->tv_nsec = 0;
    }
}

void
timer_start(struct timer *t)
{
    struct timespec ts;

    ASSERT(t != NULL);

    _timer_gettime(&ts);
    t->started = true;
    t->start = (uint64_t)ts.tv_sec * NSEC_PER_SEC + (uint64_t)ts.tv_nsec;
}

void
timer_stop(struct timer *t)
{
    struct timespec ts;

    ASSERT(t != NULL);

    _timer_gettime(&ts);
    t->stopped = true;
    t->stop = (uint64_t)ts.tv_sec * NSEC_PER_SEC + (uint64_t)ts.tv_nsec;
}

double
timer_duration_ns(struct timer *t)
{
    ASSERT(t != NULL);
    ASSERT(t->started && t->stopped);
    /*
     * Note(yao): given the uncertainty on cid and clock_gettime return status,
     * we cannot guarantee that t->start will be less than or equal to t->stop
     * even if the timer is used correctly, so we may get some weird readings.
     */

    return (double)t->stop - (double)t->start;
}

double
timer_duration_us(struct timer *t)
{
    return timer_duration_ns(t) / NSEC_PER_USEC;
}

double
timer_duration_ms(struct timer *t)
{
    return timer_duration_ns(t) / NSEC_PER_MSEC;
}

double
timer_duration_sec(struct timer *t)
{
    return timer_duration_ns(t) / NSEC_PER_SEC;
}
