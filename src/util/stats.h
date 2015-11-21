#pragma once

#include <stddef.h>

struct metric;

extern struct metric *gs;       /* global stats struct as a metric array */

#define GLOB_STATS_GET(_n) (gs + (_n)) /* get the _nth metric in gs */

size_t stats_card(void);        /* get # metrics in global stats struct */
