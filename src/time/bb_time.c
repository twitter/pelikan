#include <cc_log.h>

#include <bb_time.h>


/*
 * From memcache protocol specification:
 *
 * Some commands involve a client sending some kind of expiration time
 * (relative to an item or to an operation requested by the client) to
 * the server. In all such cases, the actual value sent may either be
 * Unix time (number of seconds since January 1, 1970, as a 32-bit
 * value), or a number of seconds starting from current time. In the
 * latter case, this number of seconds may not exceed 60*60*24*30 (number
 * of seconds in 30 days); if the number sent by a client is larger than
 * that, the server will consider it to be real Unix time value rather
 * than an offset from current time.
 */
#define TIME_MAXDELTA   (time_t)(60 * 60 * 24 * 30)

/*
 * Time when process was started expressed as absolute unix timestamp
 * with a time_t type
 */
static time_t time_start;

/*
 * We keep a cache of the current time of day in a global variable now
 * that is updated periodically by a timer event every second. This
 * saves us a bunch of time() system calls because we really only need
 * to get the time once a second, whereas there can be tens of thosands
 * of requests a second.
 *
 * Also keeping track of time as relative to server-start timestamp
 * instead of absolute unix timestamps gives us a space savings on
 * systems where sizeof(time_t) > sizeof(unsigned int)
 *
 * So, now actually holds 32-bit seconds since the server start time.
 */
static volatile rel_time_t now;

void
time_update(void)
{
    int status;
    struct timeval timer;

    status = gettimeofday(&timer, NULL);
    if (status < 0) {
	log_debug(LOG_WARN, "gettimeofday failed!");
    }
    now = (rel_time_t) (timer.tv_sec - time_start);

    log_debug(LOG_VERB, "time updated to %u\n", now);
}

rel_time_t
time_now(void)
{
    return now;
}

time_t
time_now_abs(void)
{
    return time_start + (time_t)now;
}

time_t
time_started(void)
{
    return time_start;
}

rel_time_t
time_reltime(time_t t)
{
    if (t == 0) { /* 0 means never expire */
        return 0;
    }

    if (t > TIME_MAXDELTA) {
        /*
         * If item expiration is at or before the server_started, give
         * it an expiration time of 1 second after the server started
         * becasue because 0 means don't expire.  Without this, we would
         * underflow and wrap around to some large value way in the
         * future, effectively making items expiring in the past
         * really expiring never
         */
        if (t <= time_start) {
            return (rel_time_t)1;
        }

        return (rel_time_t)(t - time_start);
    } else {
        return (rel_time_t)(t + now);
    }
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

    log_debug(LOG_INFO, "timer started at %"PRIu64"(2 sec manual setback)",
            (uint64_t)time_start);
}

void
time_teardown(void)
{
    log_debug(LOG_INFO, "timer ended at %"PRIu64, (uint64_t)time(NULL));
}
