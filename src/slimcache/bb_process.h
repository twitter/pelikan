#ifndef _BB_PROCESS_H_
#define _BB_PROCESS_H_

#include <memcache/bb_codec.h>

/*          name                type            description */
#define PROCESS_METRIC(ACTION)                                          \
    ACTION( get_key,            METRIC_COUNTER, "# keys by get"        )\
    ACTION( get_key_hit,        METRIC_COUNTER, "# key hits by get"    )\
    ACTION( get_key_miss,       METRIC_COUNTER, "# key misses by get"  )\
    ACTION( gets_key,           METRIC_COUNTER, "# keys by gets"       )\
    ACTION( gets_key_hit,       METRIC_COUNTER, "# key hits by gets"   )\
    ACTION( gets_key_miss,      METRIC_COUNTER, "# key misses by gets" )\
    ACTION( delete_deleted,     METRIC_COUNTER, "# delete successes"   )\
    ACTION( delete_notfound,    METRIC_COUNTER, "# delete not_founds"  )\
    ACTION( set_stored,         METRIC_COUNTER, "# set successes"      )\
    ACTION( set_notstored,      METRIC_COUNTER, "# set failures"       )\
    ACTION( add_stored,         METRIC_COUNTER, "# add successes"      )\
    ACTION( add_notstored,      METRIC_COUNTER, "# add failures"       )\
    ACTION( replace_stored,     METRIC_COUNTER, "# replace successes"  )\
    ACTION( replace_notstored,  METRIC_COUNTER, "# replace failures"   )\
    ACTION( cas_sotred,         METRIC_COUNTER, "# cas successes"      )\
    ACTION( cas_exists,         METRIC_COUNTER, "# cas bad values"     )\
    ACTION( cas_notfound,       METRIC_COUNTER, "# cas not_founds"     )\
    ACTION( incr_stored,        METRIC_COUNTER, "# incr successes"     )\
    ACTION( incr_notfound,      METRIC_COUNTER, "# incr not_founds"    )\
    ACTION( decr_stored,        METRIC_COUNTER, "# incr successes"     )\
    ACTION( decr_notfound,      METRIC_COUNTER, "# incr not_founds"    )


rstatus_t process_request(struct request *req, struct mbuf *buf);

#endif /* _BB_PROCESS_H_ */
