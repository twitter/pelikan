#pragma once

#include <stddef.h>

struct metric;

extern struct metric *gs;       /* global stats struct as a metric array */

static inline struct metric *
glob_stats_get(size_t n)
{
    return gs + n;
}

size_t stats_card(void);        /* get # metrics in global stats struct */
