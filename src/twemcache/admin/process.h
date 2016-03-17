#pragma once

#include <cc_metric.h>

/*          name                        type            description */
#define ADMIN_PROCESS_METRIC(ACTION)                                    \
    ACTION( stats,             METRIC_COUNTER, "# stats requests"      )\
    ACTION( stats_ex,          METRIC_COUNTER, "# stats errors"        )\
    ACTION( version,           METRIC_COUNTER, "# version requests"    )

typedef struct {
    ADMIN_PROCESS_METRIC(METRIC_DECLARE)
} admin_process_metrics_st;

void admin_process_setup(admin_process_metrics_st *metrics);
void admin_process_teardown(void);
