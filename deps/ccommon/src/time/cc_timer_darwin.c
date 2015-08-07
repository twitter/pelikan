#include <cc_timer.h>

#include <cc_debug.h>

#include <stdlib.h>
#include <mach/mach_time.h>

static mach_timebase_info_data_t info;
static bool info_init = false;

void
timer_reset(struct timer *t)
{
    ASSERT(t != NULL);

    t->started = false;
    t->stopped = false;
    t->start = 0;
    t->stop = 0;
}

void
timer_start(struct timer *t)
{
    ASSERT(t != NULL);

    t->started = true;
    t->start = mach_absolute_time();
}

void
timer_stop(struct timer *t)
{
    ASSERT(t != NULL);

    t->stopped = true;
    t->stop = mach_absolute_time();
}

double
timer_duration_ns(struct timer *t)
{
    uint64_t nelapsed;

    ASSERT(t != NULL);
    ASSERT(t->started && t->stopped);
    ASSERT(t->stop >= t->start);

    if (!info_init) {
        mach_timebase_info(&info);
    }

    nelapsed = t->stop - t->start;

    return (double)nelapsed * info.numer / info.denom;
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
