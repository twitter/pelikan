#pragma once

#include <cc_define.h>
#include <cc_metric.h>

/*          name                        type            description */
#define CMD_PROCESS_METRIC(ACTION)                                         \
    ACTION( cmd_process,           METRIC_COUNTER, "# commands processed" )\
    ACTION( cmd_get_key,           METRIC_COUNTER, "# keys by get"        )\
    ACTION( cmd_get_key_hit,       METRIC_COUNTER, "# key hits by get"    )\
    ACTION( cmd_get_key_miss,      METRIC_COUNTER, "# key misses by get"  )\
    ACTION( cmd_gets_key,          METRIC_COUNTER, "# keys by gets"       )\
    ACTION( cmd_gets_key_hit,      METRIC_COUNTER, "# key hits by gets"   )\
    ACTION( cmd_gets_key_miss,     METRIC_COUNTER, "# key misses by gets" )\
    ACTION( cmd_delete_deleted,    METRIC_COUNTER, "# delete successes"   )\
    ACTION( cmd_delete_notfound,   METRIC_COUNTER, "# delete not_founds"  )\
    ACTION( cmd_set_stored,        METRIC_COUNTER, "# set successes"      )\
    ACTION( cmd_set_ex,            METRIC_COUNTER, "# set errors"         )\
    ACTION( cmd_add_stored,        METRIC_COUNTER, "# add successes"      )\
    ACTION( cmd_add_notstored,     METRIC_COUNTER, "# add failures"       )\
    ACTION( cmd_add_ex,            METRIC_COUNTER, "# add errors"         )\
    ACTION( cmd_replace_stored,    METRIC_COUNTER, "# replace successes"  )\
    ACTION( cmd_replace_notstored, METRIC_COUNTER, "# replace failures"   )\
    ACTION( cmd_replace_ex,        METRIC_COUNTER, "# replace errors"     )\
    ACTION( cmd_cas_stored,        METRIC_COUNTER, "# cas successes"      )\
    ACTION( cmd_cas_exists,        METRIC_COUNTER, "# cas bad values"     )\
    ACTION( cmd_cas_notfound,      METRIC_COUNTER, "# cas not_founds"     )\
    ACTION( cmd_cas_ex,            METRIC_COUNTER, "# cas errors"         )\
    ACTION( cmd_incr_stored,       METRIC_COUNTER, "# incr successes"     )\
    ACTION( cmd_incr_notfound,     METRIC_COUNTER, "# incr not_founds"    )\
    ACTION( cmd_incr_ex,           METRIC_COUNTER, "# incr errors"        )\
    ACTION( cmd_decr_stored,       METRIC_COUNTER, "# decr successes"     )\
    ACTION( cmd_decr_notfound,     METRIC_COUNTER, "# decr not_founds"    )\
    ACTION( cmd_decr_ex,           METRIC_COUNTER, "# decr errors"        )

typedef struct {
    CMD_PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

#define PROCESS_METRIC_INIT(_metrics) do {                                  \
    *(_metrics) = (process_metrics_st) { CMD_PROCESS_METRIC(METRIC_INIT) }; \
} while(0)

struct request;
struct buf;

void process_setup(process_metrics_st *process_metrics);
void process_teardown(void);


/* Returns the response lengh in bytes, or an error code if negative */
int process_request(struct request *req, struct buf **buf);
