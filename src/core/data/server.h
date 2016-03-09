#pragma once

#include <cc_define.h>
#include <cc_metric.h>

/*          name                    type            description */
#define CORE_SERVER_METRIC(ACTION)                                                   \
    ACTION( server_event_total,     METRIC_COUNTER, "# server events returned"      )\
    ACTION( server_event_loop,      METRIC_COUNTER, "# server event loops returned" )\
    ACTION( server_event_read,      METRIC_COUNTER, "# server core_read events"     )\
    ACTION( server_event_write,     METRIC_COUNTER, "# server core_write events"    )\
    ACTION( server_event_error,     METRIC_COUNTER, "# server core_error events"    )

typedef struct {
    CORE_SERVER_METRIC(METRIC_DECLARE)
} server_metrics_st;

#define SERVER_METRIC_INIT(_metrics) do {                                  \
    *(_metrics) = (server_metrics_st) { CORE_SERVER_METRIC(METRIC_INIT) }; \
} while(0)

struct addrinfo;

rstatus_i core_server_setup(struct addrinfo *ai, server_metrics_st *metrics);
void core_server_teardown(void);
void core_server_evloop(void);
