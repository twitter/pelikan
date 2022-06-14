#pragma once

#include "item.h"

#include <cc_define.h>
#include <cc_bstring.h>

#include <inttypes.h>
#include <stdbool.h>

#define CUCKOO_POLICY_RANDOM 1
#define CUCKOO_POLICY_EXPIRE 2

#define CUCKOO_DISPLACE 2
#define CUCKOO_ITEM_CAS true
#define CUCKOO_ITEM_SIZE 64
#define CUCKOO_NITEM 1024
#define CUCKOO_POLICY CUCKOO_POLICY_RANDOM
#define CUCKOO_MAX_TTL (30 * 24 * 60 * 60) /* 30 days */
#define CUCKOO_DATAPOOL NULL
#define CUCKOO_DATAPOOL_NAME "cuckoo_datapool"
#define CUCKOO_PREFAULT false

/*          name                      type                default                  description */
#define CUCKOO_OPTION(ACTION)                                                                          \
    ACTION( cuckoo_displace,          OPTION_TYPE_UINT,   CUCKOO_DISPLACE,         "# displaces allowed"   )\
    ACTION( cuckoo_item_cas,          OPTION_TYPE_BOOL,   CUCKOO_ITEM_CAS,         "support cas in items"  )\
    ACTION( cuckoo_item_size,         OPTION_TYPE_UINT,   CUCKOO_ITEM_SIZE,        "item size (inclusive)" )\
    ACTION( cuckoo_nitem,             OPTION_TYPE_UINT,   CUCKOO_NITEM,            "# items allocated"     )\
    ACTION( cuckoo_policy,            OPTION_TYPE_UINT,   CUCKOO_POLICY,           "evict policy"          )\
    ACTION( cuckoo_max_ttl,           OPTION_TYPE_UINT,   CUCKOO_MAX_TTL,          "max ttl in seconds"    )\
    ACTION( cuckoo_datapool,          OPTION_TYPE_STR,    CUCKOO_DATAPOOL,         "path to data pool"     )\
    ACTION( cuckoo_datapool_name,     OPTION_TYPE_STR,    CUCKOO_DATAPOOL_NAME,    "cuckoo datapool name"  )\
    ACTION( cuckoo_datapool_prefault, OPTION_TYPE_BOOL,   CUCKOO_PREFAULT,         "prefault data pool"    )


typedef struct {
    CUCKOO_OPTION(OPTION_DECLARE)
} cuckoo_options_st;

/*          name            type            description */
#define CUCKOO_METRIC(ACTION)                                           \
    ACTION( cuckoo_get,         METRIC_COUNTER, "# cuckoo lookups"     )\
    ACTION( cuckoo_insert,      METRIC_COUNTER, "# cuckoo inserts"     )\
    ACTION( cuckoo_insert_ex,   METRIC_COUNTER, "# insert errors"      )\
    ACTION( cuckoo_displace,    METRIC_COUNTER, "# displacements"      )\
    ACTION( cuckoo_update,      METRIC_COUNTER, "# cuckoo updates"     )\
    ACTION( cuckoo_update_ex,   METRIC_COUNTER, "# update errors"      )\
    ACTION( cuckoo_delete,      METRIC_COUNTER, "# cuckoo deletes"     )\
    ACTION( item_val_curr,      METRIC_GAUGE,   "#B stored in vals"    )\
    ACTION( item_key_curr,      METRIC_GAUGE,   "#B stored in keys"    )\
    ACTION( item_data_curr,     METRIC_GAUGE,   "#B stored"            )\
    ACTION( item_curr,          METRIC_GAUGE,   "# items"              )\
    ACTION( item_displace,      METRIC_COUNTER, "# displace of items"  )\
    ACTION( item_evict,         METRIC_COUNTER, "# evicted items"      )\
    ACTION( item_expire,        METRIC_COUNTER, "# expired items"      )\
    ACTION( item_insert,        METRIC_COUNTER, "# item inserts"       )\
    ACTION( item_delete,        METRIC_COUNTER, "# item deletes"       )


typedef struct {
    CUCKOO_METRIC(METRIC_DECLARE)
} cuckoo_metrics_st;

extern cuckoo_metrics_st *cuckoo_metrics;

void cuckoo_setup(cuckoo_options_st *options, cuckoo_metrics_st *metrics);
void cuckoo_teardown(void);
void cuckoo_reset(void);

struct item * cuckoo_get(struct bstring *key);
struct item * cuckoo_insert(struct bstring *key, struct val *val, proc_time_i expire);
rstatus_i cuckoo_update(struct item *it, struct val *val, proc_time_i expire);
bool cuckoo_delete(struct bstring *key);
