#pragma once

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>

/*          name            type                default         description */
#define SERVER_OPTION(ACTION)                                                                   \
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "interfaces listening on"      )\
    ACTION( server_port,    OPTION_TYPE_STR,    "12321",        "port listening on"            )\
    ACTION( server_timeout, OPTION_TYPE_UINT,   100,            "evwait timeout"               )\
    ACTION( server_nevent,  OPTION_TYPE_UINT,   1024,           "evwait max nevent returned"   )

typedef struct {
    SERVER_OPTION(OPTION_DECLARE)
} server_options_st;

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

rstatus_i core_server_setup(server_options_st *options, server_metrics_st *metrics);
void core_server_teardown(void);
void core_server_evloop(void);
