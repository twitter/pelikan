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
#include <sys/time.h>

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

/* Update the current time */
void time_update(void);

/* Get the current time (since process started) */
rel_time_t time_now(void);

/* Get the current absolute time (not time since process began) */
time_t time_now_abs(void);

/* Get the time the process started */
time_t time_started(void);

/* Get time relative to process start given absolute time */
rel_time_t time_reltime(time_t t);

/* Set up: record process start time */
void time_setup(void);
void time_teardown(void);

#endif
