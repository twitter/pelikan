#ifndef _BB_PROCESS_H_
#define _BB_PROCESS_H_

#include <protocol/memcache/bb_codec.h>

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
    ACTION( set_error,          METRIC_COUNTER, "# set errors"         )\
    ACTION( add_stored,         METRIC_COUNTER, "# add successes"      )\
    ACTION( add_notstored,      METRIC_COUNTER, "# add failures"       )\
    ACTION( add_error,          METRIC_COUNTER, "# add errors"         )\
    ACTION( replace_stored,     METRIC_COUNTER, "# replace successes"  )\
    ACTION( replace_notstored,  METRIC_COUNTER, "# replace failures"   )\
    ACTION( replace_error,      METRIC_COUNTER, "# replace errors"     )\
    ACTION( cas_stored,         METRIC_COUNTER, "# cas successes"      )\
    ACTION( cas_exists,         METRIC_COUNTER, "# cas bad values"     )\
    ACTION( cas_notfound,       METRIC_COUNTER, "# cas not_founds"     )\
    ACTION( cas_error,          METRIC_COUNTER, "# cas errors"         )\
    ACTION( incr_stored,        METRIC_COUNTER, "# incr successes"     )\
    ACTION( incr_notfound,      METRIC_COUNTER, "# incr not_founds"    )\
    ACTION( incr_error,         METRIC_COUNTER, "# incr errors"        )\
    ACTION( decr_stored,        METRIC_COUNTER, "# decr successes"     )\
    ACTION( decr_notfound,      METRIC_COUNTER, "# decr not_founds"    )\
    ACTION( decr_error,         METRIC_COUNTER, "# decr errors"        )


rstatus_t process_request(struct request *req, struct mbuf *buf);

#endif /* _BB_PROCESS_H_ */
