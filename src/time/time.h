#pragma once

#include <stdint.h>
#include <time.h>

#include <cc_debug.h>
#include <cc_option.h>

/*********
 * Types *
 *********
 *
 * proc_time types are intended for timestamps compared to process start.
 * delta_time types are intended for timestamps compared to time now.
 * unix_time types are intended for unix timestamps.
 * memcache_time types are intended for memcache compatible timestamps.
 * time types are ambiguous, and treated depending on timestamp type setting.
 *
 * For less granular time, regular type will suffice and gives space savings.
 * For more granular time, fine type gives additional precision.
 *
 */
typedef int32_t proc_time_i;
typedef int64_t proc_time_fine_i;
typedef int32_t delta_time_i;
typedef int64_t delta_time_fine_i;
typedef uint32_t unix_time_u;
typedef uint64_t unix_time_fine_u;
typedef uint32_t memcache_time_u;
typedef uint64_t memcache_time_fine_u;
typedef int32_t time_i;
typedef int64_t time_fine_i;

/***********
 * Options *
 ***********
 *
 * How to handle expiry timestamps. These are converted to time relative to
 * process start.
 *
 * In unix timestamp only mode, timestamps are treated as absolute unix
 * timestamps, and time_convert_proc will return the difference between the
 * timestamp and the timestamp of when the server came up.
 *
 * In delta timestamp only mode, timestamps are treated as time relative to time
 * now.
 *
 * In memcached compatibility mode, timestamps are treated as they are in
 * memcache, that is, if it is greater than 30 days, it is treated as a unix
 * timestamp; otherwise, it is treated as a delta timestamp. From memcache
 * protocol specification:
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
enum {
    TIME_UNIX     = 0,
    TIME_DELTA    = 1,
    TIME_MEMCACHE = 2,
    TIME_SENTINEL = 3
};

/*          name          type                default       description */
#define TIME_OPTION(ACTION) \
    ACTION( time_type,    OPTION_TYPE_UINT,   TIME_UNIX,    "Expiry timestamp mode" )

typedef struct {
    TIME_OPTION(OPTION_DECLARE)
} time_options_st;

/* Exposed for inlining functions. Do NOT touch directly. */
extern uint8_t time_type;

/*********
 * State *
 *********/

/*
 * Time when the process was started expressed as absolute unix timestamp
 */
extern time_t time_start;

/*
 * Current time relative to process start. These are updated with each call
 * to time_update(). Do NOT use these directly; instead use the API provided
 * below.
 */
extern proc_time_i proc_sec;
extern proc_time_fine_i proc_ms;
extern proc_time_fine_i proc_us;
extern proc_time_fine_i proc_ns;

/*******
 * API *
 *******/

#define NSEC_PER_SEC    1000000000L
#define USEC_PER_SEC       1000000L
#define MSEC_PER_SEC          1000L

/*
 * Unix timestamp at which the process was started
 */
static inline time_t
time_started(void)
{
    return __atomic_load_n(&time_start, __ATOMIC_RELAXED);
}

/*
 * Current time since the process started
 */
static inline proc_time_i
time_proc_sec(void)
{
    return __atomic_load_n(&proc_sec, __ATOMIC_RELAXED);
}

static inline proc_time_fine_i
time_proc_ms(void)
{
    return __atomic_load_n(&proc_ms, __ATOMIC_RELAXED);
}

static inline proc_time_fine_i
time_proc_us(void)
{
    return __atomic_load_n(&proc_us, __ATOMIC_RELAXED);
}

static inline proc_time_fine_i
time_proc_ns(void)
{
    return __atomic_load_n(&proc_ns, __ATOMIC_RELAXED);
}

/*
 * Current unix timestamp
 */
static inline time_t  /* time_t is used for compatibility with time_started() */
time_unix_sec(void)
{
    return time_started() + time_proc_sec();
}

static inline unix_time_fine_u
time_unix_ms(void)
{
    return time_started() * MSEC_PER_SEC + time_proc_ms();
}

static inline unix_time_fine_u
time_unix_us(void)
{
    return time_started() * USEC_PER_SEC + time_proc_us();
}

static inline unix_time_fine_u
time_unix_ns(void)
{
    return time_started() * NSEC_PER_SEC + time_proc_ns();
}

/*
 * Unix time conversion to time since proc started.
 *
 * NOTE: A return value of 0 does NOT mean forever, as it does in memcache. This
 *       is because the storage modules no longer treat 0 as never expire.
 *       Instead, an input of 0 for time_memcache is converted to max int.
 */
static inline proc_time_i
time_unix2proc_sec(unix_time_u t)
{
    return (proc_time_i)(t - time_started());
}

static inline proc_time_fine_i
time_unix2proc_ms(unix_time_fine_u t)
{
    return (proc_time_i)(t - (time_started() * MSEC_PER_SEC));
}

static inline proc_time_fine_i
time_unix2proc_us(unix_time_fine_u t)
{
    return (proc_time_i)(t - (time_started() * USEC_PER_SEC));
}

static inline proc_time_fine_i
time_unix2proc_ns(unix_time_fine_u t)
{
    return (proc_time_i)(t - (time_started() * NSEC_PER_SEC));
}

/*
 * Time from now conversion to time since proc started
 */
static inline proc_time_i
time_delta2proc_sec(delta_time_i t)
{
    return (proc_time_i)t + time_proc_sec();
}

static inline proc_time_fine_i
time_delta2proc_ms(delta_time_fine_i t)
{
    return (proc_time_fine_i)t + time_proc_ms();
}

static inline proc_time_fine_i
time_delta2proc_us(delta_time_fine_i t)
{
    return (proc_time_fine_i)t + time_proc_us();
}

static inline proc_time_fine_i
time_delta2proc_ns(delta_time_fine_i t)
{
    return (proc_time_fine_i)t + time_proc_ns();
}

/*
 * Memcache timestamp conversion to time since proc started. For compatibility
 * with the memcache protocol, a timestamp of 0 is converted to max int.
 */
#define TIME_MEMCACHE_MAXDELTA_SEC  (time_t)(60 * 60 * 30 * 24)
#define TIME_MEMCACHE_MAXDELTA_MS   (time_t)(60 * 60 * 30 * 24 * MSEC_PER_SEC)
#define TIME_MEMCACHE_MAXDELTA_US   (time_t)(60 * 60 * 30 * 24 * USEC_PER_SEC)
#define TIME_MEMCACHE_MAXDELTA_NS   (time_t)(60 * 60 * 30 * 24 * NSEC_PER_SEC)

static inline proc_time_i
time_memcache2proc_sec(memcache_time_u t)
{
    if (t == 0) {
        return INT32_MAX;
    }

    if (t > TIME_MEMCACHE_MAXDELTA_SEC) {
        return time_unix2proc_sec((unix_time_u)t);
    } else {
        return time_delta2proc_sec((delta_time_i)t);
    }
}

static inline proc_time_fine_i
time_memcache2proc_ms(memcache_time_fine_u t)
{
    if (t == 0) {
        return INT64_MAX;
    }

    if (t > TIME_MEMCACHE_MAXDELTA_MS) {
        return time_unix2proc_ms((unix_time_fine_u)t);
    } else {
        return time_delta2proc_ms((delta_time_fine_i)t);
    }
}

static inline proc_time_fine_i
time_memcache2proc_us(memcache_time_fine_u t)
{
    if (t == 0) {
        return INT64_MAX;
    }

    if (t > TIME_MEMCACHE_MAXDELTA_US) {
        return time_unix2proc_us((unix_time_fine_u)t);
    } else {
        return time_delta2proc_us((delta_time_fine_i)t);
    }
}

static inline proc_time_fine_i
time_memcache2proc_ns(memcache_time_fine_u t)
{
    if (t == 0) {
        return INT64_MAX;
    }

    if (t > TIME_MEMCACHE_MAXDELTA_NS) {
        return time_unix2proc_ns((unix_time_fine_u)t);
    } else {
        return time_delta2proc_ns((delta_time_fine_i)t);
    }
}

/*
 * Convert given timestamp to time since process started, depending on timestamp
 * mode.
 */
static inline proc_time_i
time_convert_proc_sec(time_i t)
{
    switch (time_type) {
    case TIME_UNIX:
        return time_unix2proc_sec((unix_time_u)t);
    case TIME_DELTA:
        return time_delta2proc_sec((delta_time_i)t);
    case TIME_MEMCACHE:
        return time_memcache2proc_sec((memcache_time_u)t);
    default:
        NOT_REACHED();
        return -1;
    }
}

static inline proc_time_i
time_convert_proc_ms(time_i t)
{
    switch (time_type) {
    case TIME_UNIX:
        return time_unix2proc_ms((unix_time_u)t);
    case TIME_DELTA:
        return time_delta2proc_ms((delta_time_i)t);
    case TIME_MEMCACHE:
        return time_memcache2proc_ms((memcache_time_u)t);
    default:
        NOT_REACHED();
        return -1;
    }
}

static inline proc_time_i
time_convert_proc_us(time_i t)
{
    switch (time_type) {
    case TIME_UNIX:
        return time_unix2proc_us((unix_time_u)t);
    case TIME_DELTA:
        return time_delta2proc_us((delta_time_i)t);
    case TIME_MEMCACHE:
        return time_memcache2proc_us((memcache_time_u)t);
    default:
        NOT_REACHED();
        return -1;
    }
}

static inline proc_time_i
time_convert_proc_ns(time_i t)
{
    switch (time_type) {
    case TIME_UNIX:
        return time_unix2proc_ns((unix_time_u)t);
    case TIME_DELTA:
        return time_delta2proc_ns((delta_time_i)t);
    case TIME_MEMCACHE:
        return time_memcache2proc_ns((memcache_time_u)t);
    default:
        NOT_REACHED();
        return -1;
    }
}

/*
 * Get current time and update current time state variables. Because time
 * objects are shared, only one thread should call time_update
 */
void time_update(void);

/*
 * Setup/teardown proc time module.
 */
void time_setup(time_options_st *options);
void time_teardown(void);
