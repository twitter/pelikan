#include <time/cc_timer.h>

#include <cc_debug.h>

#include <stdlib.h>
#include <mach/mach_time.h>

/* Note: mach_absolute_time() is essentially unit-less and should always be
 * used with mach_timebase_info_data_t
 *
 * For more information, see the following link:
 * https://developer.apple.com/library/mac/qa/qa1398/_index.html
 *
 * Internally, we are storing all timestamps as the absolute time returned by
 * this function, and the value should never be used directly as physical time.
 */

static mach_timebase_info_data_t info;

/* nanosecond-to-mach-time conversion */
static inline int64_t
_n2m(int64_t nano)
{
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }

    return nano * info.denom / info.numer;
}

/* mach-time-to-nanosecond conversion */
static inline int64_t
_m2n(int64_t mt)
{
    if (info.denom == 0) {
        mach_timebase_info(&info);
    }

    return mt * info.numer / info.denom;
}

/* duration related */

void
duration_reset(struct duration *d)
{
    ASSERT(d != NULL);

    d->started = false;
    d->stopped = false;
    d->start = 0;
    d->stop = 0;
}

void
duration_snapshot(struct duration *s, const struct duration *d)
{
    ASSERT(s != 0 && d != NULL);

    s->started = true;
    s->start = d->start;
    s->stopped = true;
    s->stop = mach_absolute_time();
}

void
duration_start(struct duration *d)
{
    ASSERT(d != NULL);

    d->started = true;
    d->start = mach_absolute_time();
}

void
duration_stop(struct duration *d)
{
    ASSERT(d != NULL);

    d->stopped = true;
    d->stop = mach_absolute_time();
}

double
duration_ns(struct duration *d)
{
    uint64_t elapsed;

    ASSERT(d != NULL);
    ASSERT(d->started && d->stopped);
    ASSERT(d->stop >= d->start);

    elapsed = d->stop - d->start;

    return (double)_m2n(elapsed);
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
    uint64_t now = mach_absolute_time();

    e->tp = now + _n2m(ns);
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
    e->tp = _n2m(ns);
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
    uint64_t now = mach_absolute_time();

    ASSERT(t->is_intvl);

    e->tp = now + t->tp;
    e->is_set = true;
    e->is_intvl = false;
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
    ASSERT(e->is_set);

    if (e->is_intvl) {
        return _m2n(e->tp);
    } else {
        uint64_t now = mach_absolute_time();

        return _m2n(e->tp - now);
    }
}

int64_t
timeout_us(struct timeout *e)
{
    /* type conversion is necessary because OSX defines NSEC_PER_USEC as ull */
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

    ts->tv_sec = ns / NSEC_PER_SEC;
    ts->tv_nsec = ns % NSEC_PER_SEC;
}

bool
timeout_expired(struct timeout *e)
{
    uint64_t now = mach_absolute_time();

    ASSERT(!e->is_intvl);

    if (!e->is_set) {
        return false;
    }

    if (e->tp <= (int64_t)now) {
        return true;
    } else {
        return false;
    }
}
