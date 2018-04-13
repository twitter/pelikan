#pragma once

#include "protocol/data/redis_include.h"

#include <buffer/cc_buf.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <stream/cc_sockio.h>

/*          name                        type            description */
#define PROCESS_METRIC(ACTION)                                          \
    ACTION( process_req,       METRIC_COUNTER, "# requests processed"  )\
    ACTION( process_ex,        METRIC_COUNTER, "# processing error"    )\
    ACTION( process_client_ex, METRIC_COUNTER, "# client error"        )\
    ACTION( process_server_ex, METRIC_COUNTER, "# server error"        )\
    ACTION( publish,           METRIC_COUNTER, "# publish requests"    )\
    ACTION( subscribe,         METRIC_COUNTER, "# subscribe requests"  )\
    ACTION( unsubscribe,       METRIC_COUNTER, "# unsubscribe requests")

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

void process_setup(process_metrics_st *metrics);
void process_teardown(void);

int pubsub_process_read(struct buf_sock *s);
int pubsub_process_write(struct buf_sock *s);
int pubsub_process_error(struct buf_sock *s);


