#ifndef _BB_TSTATS_H_
#define _BB_TSTATS_H_

/* (kyang) Very minimal stats for now, only included so the codec/request modules
   link properly. Full stats will be implemented later. */

#include <protocol/memcache/bb_codec.h>
#include <protocol/memcache/bb_request.h>

#include <cc_define.h>
#include <cc_metric.h>

#include <stddef.h>

#define STATS(ACTION)      \
    CODEC_METRIC(ACTION)   \
    REQUEST_METRIC(ACTION)

struct stats {
    STATS(METRIC_DECLARE)
};

extern struct stats Stats;
extern const unsigned int Nmetric;

#define METRIC_BASE (void *)&Stats
#define METRIC_PTR(_c) (struct metric *)(METRIC_BASE + offsetof(struct stats, _c))
#define INCR_N(_c, _d) do {                     \
    metric_incr_n(METRIC_PTR(_c), _d);          \
} while(0)
#define INCR(_c) INCR_N(_c, 1)
#define DECR_N(_c, _d) do {                     \
    metric_decr_n(METRIC_PTR(_c), _d);          \
} while(0)
#define DECR(_c) DECR_N(_c, 1)

#endif /* _BB_TSTATS_H_ */
