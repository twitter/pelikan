/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdbool.h>
#include <stdint.h>
#include <time.h>

struct duration;  /* data structure to measure duration */

/* we declare duration and timeout in the header so static allocation is
 * possible. This is less than ideal from a clean abstraction point of view,
 * and the reason that a compromise is made for performance/efficiency reasons.
 * Mostly we want data structures related to time to be light-weight, so they
 * can be used whenever applicable, and the only way to avoid calling malloc
 * or equivalent, i.e. allocating on the stack, is to have static types.
 *
 * Users should NOT access these members directly.
 *
 * For duration: we use different declarations to stay in line with system APIs
 *
 * For timeouts: using uint64_t to represent the timeout timestamp seems
 * straightforward with most OSes. It is going to be slightly trickier with
 * Windows, which we currently don't support.
 * Reference: http://nadeausoftware.com/articles/2012/04/c_c_tip_how_measure_elapsed_real_time_benchmarking
 *
 * different implementations may interpret the tp (timestamp) field differently:
 * - on most POSIX-like platforms it means nanoseconds since an unspecified point;
 * - on OS X it means `mach time units' since an unspecified point, the
 *   relationship between this unit and nanosecond can be obtained via another
 *   syscall
 */
#ifdef OS_DARWIN
struct duration {
    bool        started;
    bool        stopped;
    uint64_t    start;
    uint64_t    stop;
};
#elif defined OS_LINUX
struct duration {
    bool            started;
    bool            stopped;
    struct timespec start;
    struct timespec stop;
};
#endif


struct timeout {
    int64_t     tp; /* the timestamp */
    bool        is_set;
    /*
     * For now we are using a single struct to describe timeout in both the
     * absolute sense- "the event happens at 20:00:00 UTC", and the relative
     * sense- "the event happens after 5 minutes from now". The former reflects
     * how clock works and how timeouts are actually triggered, which deals with
     * time in the absolute sense (that is, after the starting point is chosen).
     * The latter reflects how caller defines timeout (5 minutes from now),
     * which has a constantly changing starting point.
     *
     * Other than libraries who are concerned with actually implementing the
     * timeouts (e.g., the timing wheel), most users should only use timeouts
     * in the relative sense. Users should set a timeout interval for their
     * timeout events, and submit that event to the corresponding library.
     */
    bool        is_intvl;
};


/* update duration */
void duration_reset(struct duration *d);
void duration_start(struct duration *d);
void duration_stop(struct duration *d);
/* read duration */
double duration_ns(struct duration *d);
double duration_us(struct duration *d);
double duration_ms(struct duration *d);
double duration_sec(struct duration *d);


/*
 * Not all possible granularity can be meaningfully used for sleep or event.
 * On many platforms, it's not realisitic to expect nanosecond-level expiration:
 * the system clock and scheduling granularity is simply too coarse.
 *
 * The internal presentation limits the maximum timeout duration set by these
 * functions- here user should assume expiration is within 2^64 nanoseconds from
 * the starting time of the monotonic clock.
 */

/*
 * I'm not a huge fan of getter/setter as a pattern, only when it makes sense:
 * 1) when the operations are non-trivial;
 * 2) when implementation details are platform-dependent.
 *
 * This is a case for 2).
 */

/* Question: do we have to worry about the overhead of getting timer values? */

void timeout_reset(struct timeout *e);
/* update timeout */
void timeout_add_ns(struct timeout *e, uint64_t ns);
void timeout_add_us(struct timeout *e, uint64_t us);
void timeout_add_ms(struct timeout *e, uint64_t ms);
void timeout_add_sec(struct timeout *e, uint64_t sec);
void timeout_add_intvl(struct timeout *e, struct timeout *t);
/* set the interval not absolute time */
void timeout_set_ns(struct timeout *e, uint64_t ns);
void timeout_set_us(struct timeout *e, uint64_t us);
void timeout_set_ms(struct timeout *e, uint64_t ms);
void timeout_set_sec(struct timeout *e, uint64_t sec);
/* return a timeout relative to an arbitrary timestamp, or sum/sub two intervals */
void timeout_sum_intvl(struct timeout *e, struct timeout *b, struct timeout *t);
void timeout_sub_intvl(struct timeout *e, struct timeout *b, struct timeout *t);
/* read timeout */
/* note we are using signed integer here:
 * - a positive return value shows the remaining time until timeout;
 * - a negative return value shows how long the timout is overdue.
 */
int64_t timeout_ns(struct timeout *e);
int64_t timeout_us(struct timeout *e);
int64_t timeout_ms(struct timeout *e);
int64_t timeout_sec(struct timeout *e);
/* Note: do not convert negative timeout values to timespec because it is
 * problematic to assign a negative value to timespec.tv_sec and use it with
 * certain Linux functions. See: https://lwn.net/Articles/394175/
 */
void timeout_timespec(struct timespec *ts, struct timeout *e);
bool timeout_expired(struct timeout *e);

/* Note(yao): The return type of duration and timeout are different when
 * querying for the same quantity (esentially intervals), this is because I
 * envision them to be used differently:
 * - duration mostly is used for bookkeeping/stats, and it is commonly used as
 *   the denominator to calculate average value or rate, and hence shouldn't be
 *   rounded off prematurely, especially when user prefers a coarser unit such
 *   as second;
 * - timeout is used to catch outliers (requests that have failed) or scheduled
 *   activities (aggregation, flush file, log rotate). It is thus subject to the
 *   time granularity of system scheduler, which is quite coarse compared to a
 *   nanosecond. So it is more important to return a type that is easy for logic
 *   operation (e.g. compare), while the precision of the return value is not
 *   crucial.
 */
#ifdef __cplusplus
}
#endif
