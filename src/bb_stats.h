#ifndef _BB_STATS_H_
#define _BB_STATS_H_

/* TODO(yao): make this target dependent */
#if defined TARGET_SLIMCACHE
#include <slimcache/bb_stats.h>
#endif

#if defined TARGET_TWEMCACHE
#include <twemcache/bb_stats.h>
#endif

#endif /* _BB_STATS_H_ */
