#pragma once

#include "item.h"
#include "hashtable.h"
#include "slabclass.h"

#include "time/time.h"

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_util.h>

#include <stdbool.h>
#include <stddef.h>

#define SLAB_MAGIC      0xdeadbeef
#define SLAB_HDR_SIZE   offsetof(struct slab, data)
#define SLAB_SIZE_MIN   ((size_t) 512)
#define SLAB_SIZE_MAX   ((size_t) (128 * MiB))
#define SLAB_SIZE       MiB
#define SLAB_MEM        (64 * MiB)
#define SLAB_PREALLOC   true
#define SLAB_EVICT_OPT  EVICT_RS
#define SLAB_USE_FREEQ  true
#define SLAB_PROFILE    NULL
#define SLAB_HASH       16
#define SLAB_USE_CAS    true
#define ITEM_SIZE_MIN   44      /* 40 bytes item overhead */
#define ITEM_SIZE_MAX   (SLAB_SIZE - SLAB_HDR_SIZE)
#define ITEM_FACTOR     1.25
#define HASH_POWER      16

/* Eviction options */
#define EVICT_NONE    0 /* throw OOM, no eviction */
#define EVICT_RS      1 /* random slab eviction */
#define EVICT_CS      2 /* lrc (least recently created) slab eviction */
#define EVICT_INVALID 4 /* go no further! */

/* The defaults here are placeholder values for now */
/*          name                type                default         description */
#define SLAB_OPTION(ACTION)                                                                          \
    ACTION( slab_size,          OPTION_TYPE_UINT,   SLAB_SIZE,      "Slab size"                     )\
    ACTION( slab_mem,           OPTION_TYPE_UINT,   SLAB_MEM,       "Max memory by slabs (byte)"    )\
    ACTION( slab_prealloc,      OPTION_TYPE_BOOL,   SLAB_PREALLOC,  "Pre-allocate slabs at setup"   )\
    ACTION( slab_evict_opt,     OPTION_TYPE_UINT,   SLAB_EVICT_OPT, "Eviction strategy"             )\
    ACTION( slab_use_freeq,     OPTION_TYPE_BOOL,   SLAB_USE_FREEQ, "Use items in free queue?"      )\
    ACTION( slab_profile,       OPTION_TYPE_STR,    SLAB_PROFILE,   "Specify entire slab profile"   )\
    ACTION( slab_item_min,      OPTION_TYPE_UINT,   ITEM_SIZE_MIN,  "Minimum item size"             )\
    ACTION( slab_item_max,      OPTION_TYPE_UINT,   ITEM_SIZE_MAX,  "Maximum item size"             )\
    ACTION( slab_item_growth,   OPTION_TYPE_FPN,    ITEM_FACTOR,    "Slab class growth factor"      )\
    ACTION( slab_use_cas,       OPTION_TYPE_BOOL,   SLAB_USE_CAS,   "Store CAS value in item"       )\
    ACTION( slab_hash_power,    OPTION_TYPE_UINT,   HASH_POWER,     "Power for lookup hash table"  )

typedef struct {
    SLAB_OPTION(OPTION_DECLARE)
} slab_options_st;

/*          name                type            description */
#define SLAB_METRIC(ACTION)                                                 \
    ACTION( slab_req,           METRIC_COUNTER, "# req for new slab"       )\
    ACTION( slab_req_ex,        METRIC_COUNTER, "# slab get exceptions"    )\
    ACTION( slab_evict,         METRIC_COUNTER, "# slabs evicted"          )\
    ACTION( slab_memory,        METRIC_GAUGE,   "memory allocated to slab" )\
    ACTION( slab_curr,          METRIC_GAUGE,   "# currently active slabs" )\
    ACTION( item_curr,          METRIC_GAUGE,   "# current items"          )\
    ACTION( item_alloc,         METRIC_COUNTER, "# items allocated"        )\
    ACTION( item_alloc_ex,      METRIC_COUNTER, "# item alloc errors"      )\
    ACTION( item_dealloc,       METRIC_COUNTER, "# items de-allocated"     )\
    ACTION( item_linked_curr,   METRIC_GAUGE,   "# current items, linked"  )\
    ACTION( item_link,          METRIC_COUNTER, "# items inserted to HT"   )\
    ACTION( item_unlink,        METRIC_COUNTER, "# items removed from HT"  )\
    ACTION( item_keyval_byte,   METRIC_GAUGE,   "key+val in bytes, linked" )\
    ACTION( item_val_byte,      METRIC_GAUGE,   "value only in bytes"      )

typedef struct {
    SLAB_METRIC(METRIC_DECLARE)
} slab_metrics_st;

/*          name                type            description */
#define PERSLAB_METRIC(ACTION)                                          \
    ACTION( chunk_size,         METRIC_GAUGE,   "# byte per item cunk" )\
    ACTION( item_keyval_byte,   METRIC_GAUGE,   "keyval stored (byte) ")\
    ACTION( item_val_byte,      METRIC_GAUGE,   "value portion of data")\
    ACTION( item_curr,          METRIC_GAUGE,   "# items stored"       )\
    ACTION( item_free,          METRIC_GAUGE,   "# free items"         )\
    ACTION( slab_curr,          METRIC_GAUGE,   "# slabs"              )

typedef struct {
    PERSLAB_METRIC(METRIC_DECLARE)
} perslab_metrics_st;

extern perslab_metrics_st perslab[SLABCLASS_MAX_ID];
extern uint8_t profile_last_id;

#define PERSLAB_INCR(id, metric) INCR(&perslab[id], metric)
#define PERSLAB_DECR(id, metric) DECR(&perslab[id], metric)
#define PERSLAB_INCR_N(id, metric, delta) INCR_N(&perslab[id], metric, delta)
#define PERSLAB_DECR_N(id, metric, delta) DECR_N(&perslab[id], metric, delta)

/*
 * Every slab (struct slab) in the cache starts with a slab header
 * followed by slab data. The slab data is essentially a collection of
 * contiguous, equal sized items (struct item)
 *
 * An item is owned by a slab and a slab is owned by a slabclass
 *
 *   <------------------------ slab_size ------------------------->
 *   +---------------+--------------------------------------------+
 *   |  slab header  |              slab data                     |
 *   | (struct slab) |      (contiguous equal sized items)        |
 *   +---------------+--------------------------------------------+
 *   ^               ^
 *   |               |
 *   \               |
 *   slab            \
 *                   slab->data
 *
 * Note: keep struct slab 8-byte aligned so that item chunks always start on
 *       8-byte aligned boundary.
 */
struct slab {
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    uint32_t          magic;        /* slab magic (const) */
#endif
    TAILQ_ENTRY(slab) s_tqe;        /* link in slab lruq */

    rel_time_t        utime;        /* last update time in secs */
    uint8_t           id;           /* slabclass id */
    uint32_t          padding:24;   /* unused */
    uint32_t          refcount;     /* # items that can't be evicted */
    uint8_t           data[1];      /* opaque data */
};

TAILQ_HEAD(slab_tqh, slab);

#define SLAB_HDR_SIZE    offsetof(struct slab, data)

extern struct hash_table *hash_table;
extern size_t slab_size;
extern slab_metrics_st *slab_metrics;

/*
 * Return the usable space for item sized chunks that would be carved out
 * of a given slab.
 */
static inline size_t
slab_capacity(void)
{
    return slab_size - SLAB_HDR_SIZE;
}

/*
 * Get the slab that contains this item.
 */
static inline struct slab *
item_to_slab(struct item *it)
{
    struct slab *slab;

    ASSERT(it->offset < slab_size);

    slab = (struct slab *)((char *)it - it->offset);

    ASSERT(slab->magic == SLAB_MAGIC);

    return slab;
}

static inline void
slab_ref(struct slab *slab)
{
    slab->refcount++;
}

static inline void
slab_deref(struct slab *slab)
{
    ASSERT(slab->refcount > 0);

    slab->refcount--;
}

void slab_print(void);
uint8_t slab_id(size_t size);

/* Calculate slab id that will accommodate item with given key/val lengths */
static inline uint8_t
item_slabid(uint8_t klen, uint32_t vlen)
{
    return slab_id(item_ntotal(klen, vlen));
}

void slab_setup(slab_options_st *options, slab_metrics_st *metrics);
void slab_teardown(void);

struct item *slab_get_item(uint8_t id);
void slab_put_item(struct item *it, uint8_t id);
