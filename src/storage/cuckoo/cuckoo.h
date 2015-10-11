#pragma once

#include <storage/cuckoo/item.h>

#include <cc_define.h>
#include <cc_bstring.h>

#include <inttypes.h>
#include <stdbool.h>

#define CUCKOO_POLICY_RANDOM 1
#define CUCKOO_POLICY_EXPIRE 2

#define CUCKOO_DISPLACE 2
#define CUCKOO_ITEM_SIZE 64
#define CUCKOO_NITEM 1024
#define CUCKOO_POLICY CUCKOO_POLICY_RANDOM

/*          name                type                default             description */
#define CUCKOO_OPTION(ACTION)                                                                    \
    ACTION( cuckoo_displace,    OPTION_TYPE_UINT,   CUCKOO_DISPLACE,    "# displaces allowed"   )\
    ACTION( cuckoo_item_cas,    OPTION_TYPE_BOOL,   true,               "support cas in items"  )\
    ACTION( cuckoo_item_size,   OPTION_TYPE_UINT,   CUCKOO_ITEM_SIZE,   "item size (inclusive)" )\
    ACTION( cuckoo_nitem,       OPTION_TYPE_UINT,   CUCKOO_NITEM,       "# items allocated"     )\
    ACTION( cuckoo_policy,      OPTION_TYPE_UINT,   CUCKOO_POLICY,      "evict policy"          )

/*          name            type            description */
#define ITEM_METRIC(ACTION)                                         \
    ACTION( item_val_curr,  METRIC_GAUGE,   "#B stored in vals"    )\
    ACTION( item_key_curr,  METRIC_GAUGE,   "#B stored in keys"    )\
    ACTION( item_data_curr, METRIC_GAUGE,   "#B stored"            )\
    ACTION( item_curr,      METRIC_GAUGE,   "# items"              )\
    ACTION( item_displace,  METRIC_COUNTER, "# displace of items"  )\
    ACTION( item_evict,     METRIC_COUNTER, "# evicted items"      )\
    ACTION( item_expire,    METRIC_COUNTER, "# expired items"      )\
    ACTION( item_insert,    METRIC_COUNTER, "# item inserts"       )\
    ACTION( item_delete,    METRIC_COUNTER, "# item deletes"       )

#define CUCKOO_METRIC(ACTION)                                       \
    ACTION( cuckoo_get,         METRIC_COUNTER, "# cuckoo lookups" )\
    ACTION( cuckoo_insert,      METRIC_COUNTER, "# cuckoo inserts" )\
    ACTION( cuckoo_insert_ex,   METRIC_COUNTER, "# insert errors"  )\
    ACTION( cuckoo_displace,    METRIC_COUNTER, "# displacements"  )\
    ACTION( cuckoo_update,      METRIC_COUNTER, "# cuckoo updates" )\
    ACTION( cuckoo_update_ex,   METRIC_COUNTER, "# update errors"  )\
    ACTION( cuckoo_delete,      METRIC_COUNTER, "# cuckoo deletes" )

/* TODO(yao): add metrics for higher-level (above item) cuckoo operations */
typedef struct {
    ITEM_METRIC(METRIC_DECLARE)
    CUCKOO_METRIC(METRIC_DECLARE)
} cuckoo_metrics_st;

#define CUCKOO_METRIC_INIT(_metrics) do {   \
    *(_metrics) = (cuckoo_metrics_st) {     \
        ITEM_METRIC(METRIC_INIT)            \
        CUCKOO_METRIC(METRIC_INIT) };       \
} while(0)

rstatus_t cuckoo_setup(size_t size, uint32_t item, uint32_t policy, bool cas, cuckoo_metrics_st *metrics);
void cuckoo_teardown(void);
void cuckoo_reset(void);

struct item * cuckoo_get(struct bstring *key);
rstatus_t cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire);
rstatus_t cuckoo_update(struct item *it, struct val *val, rel_time_t expire);
bool cuckoo_delete(struct bstring *key);
