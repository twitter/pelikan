#pragma once

#include "core/context.h"

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>

#define PUBSUB_TIMEOUT   100     /* in ms */
#define PUBSUB_NEVENT    1024

/*          name            type                default         description */
#define PUBSUB_OPTION(ACTION)                                                                   \
    ACTION( pubsub_timeout, OPTION_TYPE_UINT,   PUBSUB_TIMEOUT, "evwait timeout"               )\
    ACTION( pubsub_nevent,  OPTION_TYPE_UINT,   PUBSUB_NEVENT,  "evwait max nevent returned"   )

typedef struct {
    PUBSUB_OPTION(OPTION_DECLARE)
} pubsub_options_st;

/*          name                    type            description */
#define CORE_PUBSUB_METRIC(ACTION)                                                   \
    ACTION( pubsub_event_total,     METRIC_COUNTER, "# pubsub events returned"      )\
    ACTION( pubsub_event_loop,      METRIC_COUNTER, "# pubsub event loops returned" )\
    ACTION( pubsub_event_read,      METRIC_COUNTER, "# pubsub core_read events"     )\
    ACTION( pubsub_event_write,     METRIC_COUNTER, "# pubsub core_write events"    )\
    ACTION( pubsub_event_error,     METRIC_COUNTER, "# pubsub core_error events"    )

typedef struct {
    CORE_PUBSUB_METRIC(METRIC_DECLARE)
} pubsub_metrics_st;

/*
 * To allow the use application-specific logic in the handling of read/write
 * events, each application is expected to implement their own versions of
 * (post) processing functions called after the channel-level read/write is done.
 *
 * Applications should set and pass their instance of processor as argument
 * to core_pubsub_evloop().
 */
struct buf_sock;
typedef int (*pubsub_fn)(struct buf_sock *);
struct pubsub_processor {
    pubsub_fn read;
    pubsub_fn write;
    pubsub_fn error;
};

extern struct context *ctx;

void core_pubsub_setup(pubsub_options_st *options, pubsub_metrics_st *metrics);
void core_pubsub_teardown(void);
void *core_pubsub_evloop(void *arg);
