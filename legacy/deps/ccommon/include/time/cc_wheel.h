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

#include <cc_event.h>
#include <cc_queue.h>
#include <time/cc_timer.h>

    /* TODO(yao): we need to ask the question of whether we want to expose
     * `struct timeout_event` and its related functions at all, given the
     * difficulty of managing the resource lifecycle in combination with
     * the arbitrary nature of callback functions.
     *
     * Keeping the existing interfaces untouched for now to minimize changes
     * introduced at once. Will revisit very soon.
     */

/*          name                    type            description */
#define TIMING_WHEEL_METRIC(ACTION)                                                 \
    ACTION( timeout_event_curr,     METRIC_GAUGE,   "# timeout events allocated"   )\
    ACTION( timeout_event_active,   METRIC_GAUGE,   "# timeout events in use"      )\
    ACTION( timeout_event_borrow,   METRIC_COUNTER, "# timeout events borrowed"    )\
    ACTION( timeout_event_borrow_ex,METRIC_COUNTER, "# tevents borrow errors"      )\
    ACTION( timeout_event_return,   METRIC_COUNTER, "# timeout events returned"    )\
    ACTION( timing_wheel_insert,    METRIC_COUNTER, "# tevent insertions"          )\
    ACTION( timing_wheel_remove,    METRIC_COUNTER, "# tevent removal"             )\
    ACTION( timing_wheel_event,     METRIC_GAUGE,   "# tevents in timing wheels"   )\
    ACTION( timing_wheel_process,   METRIC_COUNTER, "# tevents processed"          )\
    ACTION( timing_wheel_tick,      METRIC_COUNTER, "# ticks processed"            )\
    ACTION( timing_wheel_exec,      METRIC_COUNTER, "# timing wheel executions "   )

typedef struct {
    TIMING_WHEEL_METRIC(METRIC_DECLARE)
} timing_wheel_metrics_st;

typedef void (*timeout_cb_fn)(void *); /* timeout callback */

/**
 * We use TAILQ because for request timeouts it is very important to have
 * low overhead removing entries, as most requests will *not* time out.
 * For background maintenance tasks, the situation is the opposite- everything
 * times out. However, the volume of such events are so low that performance
 * or storage efficiency is not a consideration when choosing data structures.
 */

/**
 * timing wheel shouldn't use very fine grain timeouts due to both scheduling
 * overhead and the batching nature of processing.
 */

/**
 * For timing wheel, we hide the definition of `struct timeout_event', a
 * practice we do not usually adopt in this project unless there is a good
 * reason to.
 * Here, the reason lies in the life-cycle management of timeout events. The
 * caller of `timing_wheel_insert' by nature cannot determine whether or when
 * the timeout event will eventually be triggered beforehand. As a result,
 * the caller has to keep reference and free up resources associated with the
 * timeout event in the callback if the timeout event is triggerred.
 *
 * Hence if the caller to create and pass in a timeout event object, it is on
 * the hook to manage the life cycle of that object. On the other hand, the
 * sole purpose of such an event is to be used with timing wheel. So if we can
 * take over the burden of managing these objects, it will simplify usage and
 * prevent memory leak caused by not freeing up timeout event objects.
 * Therefore, by hiding `struct timeout_event's definition, caller can only get
 * a reference to it, which can be used to delete the timeout if desired. Other
 * than that, caller need not worry about the life cycle of these objects. If
 * the caller is confident that the timeout event will be triggered, it can even
 * ignore the reference returned at insertion.
 *
 * For insertions that may or may not be removed before due, the caller is
 * expected to clear the pointer returned in the callback, but _not_ attempt to
 * remove it (since it is already removed).
 */

/**
 * Recurring events, by definition, are never removed unless the service is
 * being shut down. In this case, the teardown logic of timing wheel will
 * properly clean up all resources tied to such events.
 */
struct timeout_event;

struct timing_wheel {
    /* basic properties of the timing wheel */
    struct timeout      tick;       /* tick interval */
    size_t              cap;        /* capacity as # ticks in the time wheel */
    size_t              max_ntick;  /* max # ticks to cover in one execution */
    /* the following is used internally */
    uint64_t            tick_ns;    /* tick in nanoseconds */
    /* state of the wheel */
    bool                active;     /* is the wheel supposed to be turning? */
    struct timeout      due;        /* next trigger time */
    size_t              curr;       /* index of current tick */
    uint64_t            nevent;     /* # of timeout_event objects in wheel */

    struct tevent_tqh   *table;     /* an array of header each points to a list
                                     * of timeouts expiring in the same tick.
                                     * table should contain exactly cap entries,
                                     * each corresponding to a TALQ for the
                                     * corresponding tick
                                     */
    /* some metrics of the most important aspects */
    uint64_t            nprocess;   /* total # timeout events processed */
    uint64_t            nexec;      /* total # executions */
    uint64_t            ntick;      /* total # ticks processed */
};

struct timing_wheel *timing_wheel_create(struct timeout *tick, size_t cap, size_t ntick);
void timing_wheel_destroy(struct timing_wheel **tw);

struct timeout_event * timing_wheel_insert(struct timing_wheel *tw, struct timeout *delay, bool recur, timeout_cb_fn cb, void *arg);
void timing_wheel_remove(struct timing_wheel *tw, struct timeout_event **tev);

void timing_wheel_start(struct timing_wheel *tw);
void timing_wheel_stop(struct timing_wheel *tw);
void timing_wheel_execute(struct timing_wheel *tw);
void timing_wheel_flush(struct timing_wheel *tw); /* triggering all, useful for teardown */

void timing_wheel_setup(timing_wheel_metrics_st *metrics);
void timing_wheel_teardown(void);

#ifdef __cplusplus
}
#endif
