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

/*          name                    type            description */
#define TIMING_WHEEL_METRIC(ACTION)                                                 \
    ACTION( timeout_event_create,   METRIC_COUNTER, "# timeout events created"     )\
    ACTION( timeout_event_create_ex,METRIC_COUNTER, "# tevents create errors"      )\
    ACTION( timeout_event_destroy,  METRIC_COUNTER, "# timeout events destroyed"   )\
    ACTION( timeout_event_curr,     METRIC_GAUGE,   "# timeout events allocated"   )\
    ACTION( timeout_event_borrow,   METRIC_COUNTER, "# timeout events borrowed"    )\
    ACTION( timeout_event_borrow_ex,METRIC_COUNTER, "# tevents borrow errors"      )\
    ACTION( timeout_event_return,   METRIC_COUNTER, "# timeout events returned"    )\
    ACTION( timeout_event_active,   METRIC_GAUGE,   "# timeout events in use"      )\
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
struct timeout_event {
    /* user provided */
    timeout_cb_fn               cb;       /* callback when timed out */
    void                        *data;    /* argument of the timeout callback */
    bool                        recur;    /* will be reinserted upon firing */
    struct timeout              delay;    /* delay */
    /* the following is used internally */
    uint64_t                    delay_ns; /* delay in nanoseconds */
    TAILQ_ENTRY(timeout_event)  tqe;      /* entry in the wheel TAILQ */
    struct timeout              to;       /* timeout/trigger time */
    size_t                      offset;   /* offset in the timing wheel */
    /* for organizing timeout events */
    STAILQ_ENTRY(timeout_event) next;     /* next timeout_event in pool */
    bool                        free;     /* is this object free to reuse? */
};

STAILQ_HEAD(tevent_sqh, timeout_event); /* corresponding header type for the STAILQ */
TAILQ_HEAD(tevent_tqh, timeout_event);   /* head type for timeout events */

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


void timeout_event_reset(struct timeout_event *t);
struct timeout_event *timeout_event_create(void);
void timeout_event_destroy(struct timeout_event **t);
struct timeout_event *timeout_event_borrow(void);
void timeout_event_return(struct timeout_event **t);

void timeout_event_pool_create(uint32_t max);
void timeout_event_pool_destroy(void);

struct timing_wheel *timing_wheel_create(struct timeout *tick, size_t cap, size_t ntick);
void timing_wheel_destroy(struct timing_wheel **tw);

rstatus_i timing_wheel_insert(struct timing_wheel *tw, struct timeout_event *tev);
void timing_wheel_remove(struct timing_wheel *tw, struct timeout_event *tev);

void timing_wheel_start(struct timing_wheel *tw);
void timing_wheel_stop(struct timing_wheel *tw);
void timing_wheel_execute(struct timing_wheel *tw);
void timing_wheel_flush(struct timing_wheel *tw); /* triggering all, useful for teardown */

void timing_wheel_setup(timing_wheel_metrics_st *metrics);
void timing_wheel_teardown(void);

#ifdef __cplusplus
}
#endif
