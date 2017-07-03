#pragma once

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>

#define WORKER_TIMEOUT   100     /* in ms */
#define WORKER_NEVENT    1024

/*          name            type                default         description */
#define WORKER_OPTION(ACTION)                                                                   \
    ACTION( worker_timeout, OPTION_TYPE_UINT,   WORKER_TIMEOUT, "evwait timeout"               )\
    ACTION( worker_nevent,  OPTION_TYPE_UINT,   WORKER_NEVENT,  "evwait max nevent returned"   )

typedef struct {
    WORKER_OPTION(OPTION_DECLARE)
} worker_options_st;

/*          name                    type            description */
#define CORE_WORKER_METRIC(ACTION)                                                   \
    ACTION( worker_event_total,     METRIC_COUNTER, "# worker events returned"      )\
    ACTION( worker_event_loop,      METRIC_COUNTER, "# worker event loops returned" )\
    ACTION( worker_event_read,      METRIC_COUNTER, "# worker core_read events"     )\
    ACTION( worker_event_write,     METRIC_COUNTER, "# worker core_write events"    )\
    ACTION( worker_event_error,     METRIC_COUNTER, "# worker core_error events"    )

typedef struct {
    CORE_WORKER_METRIC(METRIC_DECLARE)
} worker_metrics_st;

/*
 * To allow the use application-specific logic in the handling of read/write
 * events, each application is expected to implement their own versions of
 * (post) processing functions called after the channel-level read/write is done.
 *
 * Applications should set and pass their instance of processor as argument
 * to core_worker_evloop().
 */
struct buf;
typedef int (*data_fn)(struct buf **, struct buf **, void **);
struct data_processor {
    data_fn read;
    data_fn write;
    data_fn error;
};

void core_worker_setup(worker_options_st *options, worker_metrics_st *metrics);
void core_worker_teardown(void);
void *core_worker_evloop(void *arg);
