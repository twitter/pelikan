#ifndef _BB_TIME_H_
#define _BB_TIME_H_

/* TODO(yao): move this into ccommon:
 *   It is common to have a TTL/age for keys in key-value store. A wrapper
 *   like this often achieves two goals: 1) it provides a process-local, cached
 *   time value so we don't need to call the relatively expensive time syscalls
 *   too often; 2) since we are already using a local timer, the zero point of
 *   the timer can be set for the process to simplify operations like timestamp
 *   comparison, expiration, etc.
 *
 *   The existing caching solutions have timestamps of vaiours granularity and
 *   definition: e.g. Redis has high resolution TTL, while memcached time is at
 *   second-level granularity. To remain protocol compatible with these imple-
 *   mentations, we may need more than one time wrapper. And even more may be
 *   added in the future to strike different balance between precision and cost.
 */
#include <inttypes.h>
#include <time.h>

/* NOTE(yao): this whole time module needs a major overhaul */

/*
 * How time works internally for memcached
 *
 * Memcached server uses a timer with second granularity, which starts as the
 * process starts, and is set to 2 initially to avoid having 0 aged items.
 *
 * On systems where size(time_t) > sizeof(uint32_t), this gives
 * us space savings over tracking absolute unix time of type time_t
 */
typedef uint32_t rel_time_t;

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
time_t time_start;

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
rel_time_t now;

/* Get the time the process started */
static inline time_t
time_started(void)
{
    return time_start;
}

/* Get the current absolute time (not time since process began) */
static inline time_t
time_now_abs(void)
{
    return time_start + (time_t)now;
}

/* Get the current time (since process started) */
static inline rel_time_t
time_now(void)
{
    return now;
}

/* Get time relative to process start given absolute time */
static inline rel_time_t
time_reltime(uint32_t t)
{
    if (t == 0) { /* 0 means never expire so we set it a very large number */
        return UINT32_MAX - 1;
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

void time_update(void);

/* Set up: record process start time, start periodic timer update */
void time_setup(void);
void time_teardown(void);

#endif
