#include <time/cc_timer.h>

#include <cc_debug.h>

#include <errno.h>
#include <float.h>
#include <string.h>
#include <unistd.h>

/* Note(yao): linux/time64.h is not included in linux kernel before version 3.17
 * So we will have to define some macros for conversion, but we won't need them
 * if we use time64.h, which should happen before year 2038 :)
 */
#if HAVE_TIME64 == 0
#define NSEC_PER_USEC  1000L
#define NSEC_PER_MSEC  1000000L
#define NSEC_PER_SEC   1000000000L
#endif

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

static inline void
_gettime(struct timespec *ts)
{
    int ret;

    ASSERT(cid != (clockid_t)-1);

    ret = clock_gettime(cid, ts);
    if (ret == -1) {
        /*
         * Note(yao): for the purpose of this module, it doesn't make much sense
         * to return an error even when the gettime call fails. So we may just
         * return an arbitrary value.
         * We still set d->started to true because we don't want to halt the
         * program due to a timer error, the purpose of these boolean fields is
         * to catch cases where people forget to start the time before stopping.
         */
        log_error("clock_gettime returns error, timer result undefined: %s",
                strerror(errno));
        ts->tv_sec = 0;
        ts->tv_nsec = 0;
    }
}


/* duration related */

void
duration_reset(struct duration *d)
{
    ASSERT(d != NULL);

    d->started = false;
    d->stopped = false;
}

void
duration_start(struct duration *d)
{
    ASSERT(d != NULL);

    _gettime(&d->start);
    d->started = true;
}

void
duration_stop(struct duration *d)
{
    ASSERT(d != NULL);

    _gettime(&d->stop);
    d->stopped = true;
}

double
duration_ns(struct duration *d)
{
    double elapsed;

    ASSERT(d != NULL);
    ASSERT(d->started && d->stopped);
    /*
     * Note(yao): given the uncertainty on cid and clock_gettime return status,
     * we cannot guarantee that d->start will be less than or equal to d->stop
     * even if the timer is used correctly, so we may get some weird readings.
     */

    /* on 32-bit systems time_t is 32-bit, it is therefore a lot easier to wrap
     * around when converting seconds to nanoseconds, here we convert the delta
     * in seconds to double first to avoid this problem
     */
    elapsed = ((double)(d->stop.tv_sec - d->start.tv_sec)) * NSEC_PER_SEC +
        d->stop.tv_nsec - d->start.tv_nsec;
    if (elapsed < 0) {
        log_error("negative duration observed due to call sequence error or "
                "clock drift/correction. Substitue with epsilon.");
        elapsed = DBL_EPSILON;
    }

    return elapsed;
}

double
duration_us(struct duration *d)
{
    return duration_ns(d) / NSEC_PER_USEC;
}

double
duration_ms(struct duration *d)
{
    return duration_ns(d) / NSEC_PER_MSEC;
}

double
duration_sec(struct duration *d)
{
    return duration_ns(d) / NSEC_PER_SEC;
}


/* timeout related */
static inline uint64_t
_now_ns(void)
{
    struct timespec now;

    _gettime(&now);
    return now.tv_sec * NSEC_PER_SEC + now.tv_nsec;

}

void
timeout_reset(struct timeout *e)
{
    ASSERT(e != NULL);

    e->tp = 0;
    e->is_set = false;
    e->is_intvl = false;
}

void
timeout_add_ns(struct timeout *e, uint64_t ns)
{
    e->tp = (int64_t)_now_ns() + ns;
    e->is_set = true;
    e->is_intvl = false;
}

void
timeout_add_us(struct timeout *e, uint64_t us)
{
    timeout_add_ns(e, us * NSEC_PER_USEC);
}

void
timeout_add_ms(struct timeout *e, uint64_t ms)
{
    timeout_add_ns(e, ms * NSEC_PER_MSEC);
}

void
timeout_add_sec(struct timeout *e, uint64_t sec)
{
    timeout_add_ns(e, sec * NSEC_PER_SEC);
}

void
timeout_set_ns(struct timeout *e, uint64_t ns)
{
    e->tp = (int64_t)ns;
    e->is_set = true;
    e->is_intvl = true;
}

void
timeout_set_us(struct timeout *e, uint64_t us)
{
    timeout_set_ns(e, us * NSEC_PER_USEC);
}

void
timeout_set_ms(struct timeout *e, uint64_t ms)
{
    timeout_set_ns(e, ms * NSEC_PER_MSEC);
}

void
timeout_set_sec(struct timeout *e, uint64_t sec)
{
    timeout_set_ns(e, sec * NSEC_PER_SEC);
}

void
timeout_add_intvl(struct timeout *e, struct timeout *t)
{
    ASSERT(t->tp >= 0); /* timeout in the past doesn't make sense */

    timeout_add_ns(e, t->tp);
}

void
timeout_sum_intvl(struct timeout *e, struct timeout *b, struct timeout *t)
{
    ASSERT(t->is_intvl);

    e->tp = b->tp + t->tp;
    e->is_set = true;
    e->is_intvl = b->is_intvl;
}

void
timeout_sub_intvl(struct timeout *e, struct timeout *b, struct timeout *t)
{
    ASSERT(t->is_intvl);

    e->tp = b->tp - t->tp;
    e->is_set = true;
    e->is_intvl = b->is_intvl;
}

int64_t
timeout_ns(struct timeout *e)
{
    if (e->is_intvl) {
        return e->tp;
    } else {
        return e->tp - _now_ns();
    }
}

int64_t
timeout_us(struct timeout *e)
{
    return timeout_ns(e) / (int64_t)NSEC_PER_USEC;
}

int64_t
timeout_ms(struct timeout *e)
{
    return timeout_ns(e) / (int64_t)NSEC_PER_MSEC;
}

int64_t
timeout_sec(struct timeout *e)
{
    return timeout_ns(e) / (int64_t)NSEC_PER_SEC;
}

void
timeout_timespec(struct timespec *ts, struct timeout *e)
{
    int64_t ns = timeout_ns(e);

    ASSERT(ns >= 0);

    ts->tv_sec = ns / (time_t)NSEC_PER_SEC;
    ts->tv_nsec = ns % NSEC_PER_SEC;
}

bool
timeout_expired(struct timeout *e)
{
    int64_t now_nano;

    ASSERT(!e->is_intvl);

    if (!e->is_set) {
        return false;
    }

    now_nano = (int64_t)_now_ns();

    if (now_nano >= e->tp) {
        return true;
    } else {
        return false;
    }
}
