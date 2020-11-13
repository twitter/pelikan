#pragma once

#include <buffer/cc_buf.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <stream/cc_sockio.h>

#define ALLOW_FLUSH false
#define PREFILL false
#define PREFILL_KSIZE 32
#define PREFILL_VSIZE 32
#define PREFILL_NKEY 400000000 /* 40M keys roughly fills up a 4GB heap with default slab & data sizes */

/*          name           type              default        description */
#define PROCESS_OPTION(ACTION)                                                         \
    ACTION( allow_flush,   OPTION_TYPE_BOOL, ALLOW_FLUSH,   "allow flush_all"         )\
    ACTION( prefill,       OPTION_TYPE_BOOL, PREFILL,       "prefill slabs with data" )\
    ACTION( prefill_ksize, OPTION_TYPE_UINT, PREFILL_KSIZE, "prefill key size"        )\
    ACTION( prefill_vsize, OPTION_TYPE_UINT, PREFILL_VSIZE, "prefill val size"        )\
    ACTION( prefill_nkey,  OPTION_TYPE_UINT, PREFILL_NKEY,  "prefill keys inserted"   )
/* prefilling can potentially follow a fairly complex config wrt key/value size
 * distribution and schema. However, basic performance testing around IO and
 * heap size can be greatly sped up without lengthy client-drive warm-up if we
 * simply fill the slabs with data of the same size.
 *
 * For now, the prefill logic will populate the heap with keys and values of
 * specified lengths, while the keys will be string representation of base-10
 * numeric values padded to the right length, i.e. keys will look like
 * "000000", "000001", ..., "123456", and prefilling logic will always start
 * from 0 and trying to insert the exact number of keys specified (underfill
 * and eviction are therefore possible) depending on how slab_mem is configured.
 */

typedef struct {
    PROCESS_OPTION(OPTION_DECLARE)
} process_options_st;

/*          name                        type            description */
#define PROCESS_METRIC(ACTION)                                          \
    ACTION( process_req,       METRIC_COUNTER, "# requests processed"  )\
    ACTION( process_ex,        METRIC_COUNTER, "# processing error"    )\
    ACTION( process_server_ex, METRIC_COUNTER, "# internal error"      )\
    ACTION( get,               METRIC_COUNTER, "# get requests"        )\
    ACTION( get_key,           METRIC_COUNTER, "# keys by get"         )\
    ACTION( get_key_hit,       METRIC_COUNTER, "# key hits by get"     )\
    ACTION( get_key_miss,      METRIC_COUNTER, "# key misses by get"   )\
    ACTION( get_ex,            METRIC_COUNTER, "# get errors"          )\
    ACTION( gets,              METRIC_COUNTER, "# gets requests"       )\
    ACTION( gets_key,          METRIC_COUNTER, "# keys by gets"        )\
    ACTION( gets_key_hit,      METRIC_COUNTER, "# key hits by gets"    )\
    ACTION( gets_key_miss,     METRIC_COUNTER, "# key misses by gets"  )\
    ACTION( gets_ex,           METRIC_COUNTER, "# gets errors"         )\
    ACTION( delete,            METRIC_COUNTER, "# delete requests"     )\
    ACTION( delete_deleted,    METRIC_COUNTER, "# delete successes"    )\
    ACTION( delete_notfound,   METRIC_COUNTER, "# delete not_founds"   )\
    ACTION( set,               METRIC_COUNTER, "# set requests"        )\
    ACTION( set_stored,        METRIC_COUNTER, "# set successes"       )\
    ACTION( set_ex,            METRIC_COUNTER, "# set errors"          )\
    ACTION( add,               METRIC_COUNTER, "# add requests"        )\
    ACTION( add_stored,        METRIC_COUNTER, "# add successes"       )\
    ACTION( add_notstored,     METRIC_COUNTER, "# add failures"        )\
    ACTION( add_ex,            METRIC_COUNTER, "# add errors"          )\
    ACTION( replace,           METRIC_COUNTER, "# replace requests"    )\
    ACTION( replace_stored,    METRIC_COUNTER, "# replace successes"   )\
    ACTION( replace_notstored, METRIC_COUNTER, "# replace failures"    )\
    ACTION( replace_ex,        METRIC_COUNTER, "# replace errors"      )\
    ACTION( cas,               METRIC_COUNTER, "# cas requests"        )\
    ACTION( cas_stored,        METRIC_COUNTER, "# cas successes"       )\
    ACTION( cas_exists,        METRIC_COUNTER, "# cas bad values"      )\
    ACTION( cas_notfound,      METRIC_COUNTER, "# cas not_founds"      )\
    ACTION( cas_ex,            METRIC_COUNTER, "# cas errors"          )\
    ACTION( incr,              METRIC_COUNTER, "# incr requests"       )\
    ACTION( incr_stored,       METRIC_COUNTER, "# incr successes"      )\
    ACTION( incr_notfound,     METRIC_COUNTER, "# incr not_founds"     )\
    ACTION( incr_ex,           METRIC_COUNTER, "# incr errors"         )\
    ACTION( decr,              METRIC_COUNTER, "# decr requests"       )\
    ACTION( decr_stored,       METRIC_COUNTER, "# decr successes"      )\
    ACTION( decr_notfound,     METRIC_COUNTER, "# decr not_founds"     )\
    ACTION( decr_ex,           METRIC_COUNTER, "# decr errors"         )\
    ACTION( append,            METRIC_COUNTER, "# append requests"     )\
    ACTION( append_stored,     METRIC_COUNTER, "# append successes"    )\
    ACTION( append_notstored,  METRIC_COUNTER, "# append not_founds"   )\
    ACTION( append_ex,         METRIC_COUNTER, "# append errors"       )\
    ACTION( prepend,           METRIC_COUNTER, "# prepend requests"    )\
    ACTION( prepend_stored,    METRIC_COUNTER, "# prepend successes"   )\
    ACTION( prepend_notstored, METRIC_COUNTER, "# prepend not_founds"  )\
    ACTION( prepend_ex,        METRIC_COUNTER, "# prepend errors"      )\
    ACTION( flush,             METRIC_COUNTER, "# flush requests"      )\
    ACTION( flushall,          METRIC_COUNTER, "# flush_all requests"  )

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

void process_setup(process_options_st *options, process_metrics_st *metrics);
void process_teardown(void);

int twemcache_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int twemcache_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
int twemcache_process_error(struct buf **rbuf, struct buf **wbuf, void **data);
