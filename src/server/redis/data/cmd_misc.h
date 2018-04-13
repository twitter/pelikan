#pragma once

#include "protocol/data/redis_include.h"

#include <buffer/cc_buf.h>
#include <cc_metric.h>

/*          name            type            description */
#define PROCESS_MISC_METRIC(ACTION)                               \
    ACTION( ping,           METRIC_COUNTER, "# ping requests"    )\
    ACTION( quit,           METRIC_COUNTER, "# quit requests"    )
