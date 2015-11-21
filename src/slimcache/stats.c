#include <slimcache/stats.h>

#include <cc_bstring.h>

struct glob_stats glob_stats;
struct metric *gs = (struct metric *)&glob_stats;

size_t
stats_card(void)
{
    return METRIC_CARDINALITY(glob_stats);
}

