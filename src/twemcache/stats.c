#include <twemcache/stats.h>

struct stats stats;
struct metric *gs = (struct metric *)&stats;

size_t
stats_card(void)
{
    return METRIC_CARDINALITY(stats);
}
