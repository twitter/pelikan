#pragma once

#include <time/time.h>

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_queue.h>
#include <cc_util.h>

#include <limits.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define SLAB_MAGIC      0xdeadbeef
#define SLAB_HDR_SIZE   offsetof(struct slab, data)
#define SLAB_MIN_SIZE   ((size_t) 512)
#define SLAB_MAX_SIZE   ((size_t) (128 * MiB))
#define SLAB_SIZE       MiB
#define SLAB_HASH       16
#define SLAB_FACTOR     1.25
#define SLAB_MIN_CHUNK  44      /* 40 bytes item overhead */
#define SLAB_MAX_CHUNK  (SLAB_SIZE - 32) /* 32 bytes slab overhead */

/* Eviction options */
#define EVICT_NONE    0 /* throw OOM, no eviction */
#define EVICT_RS      1 /* random slab eviction */
#define EVICT_CS      2 /* lrc (least recently created) slab eviction */
#define EVICT_INVALID 4 /* go no further! */

/* The defaults here are placeholder values for now */
/*          name                  type                default              description */
#define SLAB_OPTION(ACTION)                                                                                   \
    ACTION( slab_prealloc,        OPTION_TYPE_BOOL,   true,                "Allocate slabs ahead of time"    )\
    ACTION( slab_evict_opt,       OPTION_TYPE_UINT,   EVICT_NONE,          "Eviction strategy"               )\
    ACTION( slab_use_freeq,       OPTION_TYPE_BOOL,   true,                "Use items in free queue?"        )\
    ACTION( slab_size,            OPTION_TYPE_UINT,   MiB,                 "Slab size"                       )\
    ACTION( slab_min_chunk_size,  OPTION_TYPE_UINT,   SLAB_MIN_CHUNK,      "Minimum chunk size"              )\
    ACTION( slab_max_chunk_size,  OPTION_TYPE_UINT,   SLAB_MAX_CHUNK,      "Maximum chunk size"              )\
    ACTION( slab_maxbytes,        OPTION_TYPE_UINT,   GiB,                 "Maximum bytes allocated"         )\
    ACTION( slab_profile,         OPTION_TYPE_STR,    NULL,                "Slab profile"                    )\
    ACTION( slab_profile_factor,  OPTION_TYPE_STR,    str(SLAB_FACTOR),    "Slab class growth factor"        )\
    ACTION( slab_use_cas,         OPTION_TYPE_BOOL,   true,                "CAS enabled for slabbed mm"      )\
    ACTION( slab_hash_power,      OPTION_TYPE_UINT,   SLAB_HASH,           "Hash power for item table"       )

/*          name                type            description */
#define SLAB_METRIC(ACTION)                                                \
    ACTION( slab_req,           METRIC_COUNTER, "# req for new slab"      )\
    ACTION( slab_req_ex,        METRIC_COUNTER, "# slab get exceptions"   )\
    ACTION( slab_heap_size,     METRIC_GAUGE,   "# slabs in slab heap"    )\
    ACTION( slab_evict,         METRIC_COUNTER, "# slabs evicted"         )\
    ACTION( slab_curr,          METRIC_GAUGE,   "# currently active slabs")

typedef struct {
    SLAB_METRIC(METRIC_DECLARE)
} slab_metrics_st;

#define SLAB_METRIC_INIT(_metrics) do {                           \
    *(_metrics) = (slab_metrics_st) { SLAB_METRIC(METRIC_INIT) }; \
} while(0)

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
    uint8_t           data[1];      /* opaque data */
};

TAILQ_HEAD(slab_tqh, slab);

#define SLAB_HDR_SIZE    offsetof(struct slab, data)

/* Queues for handling items */
struct item;
SLIST_HEAD(item_slh, item);

/*
 * Every class (struct slabclass) is a collection of slabs that can serve
 * items of a given maximum size. Every slab in the cache is identified by a
 * unique unsigned 8-bit id, which also identifies its owner slabclass
 *
 * Slabs that belong to a given class are reachable through slabq. Slabs
 * across all classes are reachable through the slabtable and slab lruq.
 *
 * We use unslabbed_free_item as a marker for the next available, unallocated
 * item in the current slab. Items that are available for reuse (i.e. allocated
 * and then freed) are kept track by free_itemq
 *
 * slabclass[]:
 *
 *  +-------------+
 *  |             |
 *  |             |
 *  |             |
 *  |   class 0   |
 *  |             |
 *  |             |
 *  |             |
 *  +-------------+
 *  |             |  ----------------------------------------------------------+
 *  |             | /                                              (last slab) |
 *  |             |/    +---------------+-------------------+    +-------------v-+-------------------+
 *  |             |     |               |                   |    |               |                   |
 *  |   class 1   |     |  slab header  |     slab data     |    |  slab header  |     slab data     |
 *  |             |     |               |                   |    |               |                   |--+
 *  |             |\    +---------------+-------------------+    +---------------+-------------------+  |
 *  |             | \                                                                                   //
 *  |             |  ----> (freeq)
 *  +-------------+
 *  |             |  -----------------+
 *  |             | /     (last slab) |
 *  |             |/    +-------------v-+-------------------+
 *  |             |     |               |                   |
 *  |   class 2   |     |  slab header  |     slab data     |
 *  |             |     |               |                   |--+
 *  |             |\    +---------------+-------------------+  |
 *  |             | \                                          //
 *  |             |  ----> (freeq)
 *  +-------------+
 *  |             |
 *  |             |
 *  .             .
 *  .    ....     .
 *  .             .
 *  |             |
 *  |             |
 *  +-------------+
 *            |
 *            |
 *            //
 */
struct slabclass {
    uint32_t        nitem;                 /* # item per slab (const) */
    size_t          size;                  /* item size (const) */

    uint32_t        nfree_itemq;           /* # free item q */
    struct item_slh free_itemq;            /* free item q */

    uint32_t        nfree_item;            /* # free item (in current slab) */
    struct item     *next_item_in_slab;    /* next free item (in current slab, not freeq) */
};

/*
 * Slabclass id is an unsigned byte. So, maximum number of slab classes
 * cannot exceeded 256
 *
 * We use id = 255 as an invalid id and id = 0 for aggregation. This means
 * that we can have at most 254 usable slab classes
 */
#define SLABCLASS_MIN_ID        1
#define SLABCLASS_MAX_ID        (UCHAR_MAX - 1)
#define SLABCLASS_INVALID_ID    UCHAR_MAX

extern size_t slab_size;
extern struct slabclass slabclass[SLABCLASS_MAX_ID + 1];  /* collection of slabs bucketed by slabclass */

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
 * Return the item size given a slab id
 */
static inline size_t
slab_item_size(uint8_t id) {
    return slabclass[id].size;
}

void slab_print(void);
uint8_t slab_id(size_t size);

rstatus_i slab_setup(size_t setup_slab_size, bool setup_prealloc, int setup_evict_opt,
                     bool setup_use_freeq, size_t setup_min_chunk_size, size_t setup_max_chunk_size,
                     size_t setup_maxbytes, char *setup_profile, char *setup_profile_factor,
                     slab_metrics_st *metrics);
void slab_teardown(void);

struct item *slab_get_item(uint8_t id);
void slab_put_item(struct item *it, uint8_t id);
