#pragma once

#include "item.h"
#include "segevict.h"
#include "datapool/datapool.h"

#include <cc_define.h>
#include <cc_itt.h>
#include <cc_util.h>
#include <time/cc_timer.h>
#include <cc_option.h>

#include <pthread.h>
#include <stdbool.h>
#include <stddef.h>


/*
 * cache space is divided into fix-sized segments,
 * each segment is connected in one doubly linked list called segment list.
 * In one segment list, all segments have similar TTLs and
 * the head of each segment list is stored in TTL bucket.
 *
 * The number of segment lists is at most MAX_TTL_BUCKET,
 * but we expect it to be much smaller than MAX_TTL_BUCKET,
 *
 * struct seg contains the metadata of each segment and the pointer to
 * the segment data, note that although the seg_header is always kept in DRAM,
 * the actual segment data can be in other storage media, such as PMem.
 * Storing the seg_header in DRAM, it requires
 * for 1 TB PMem cache, 56 B * (1 TiB / 1 MiB) = 56 MB DRAM
 *
 *
 *                                                       DRAM External storage
 * such as PMem (optional)
 *                                              +---------------------+
 *                                              |                     |
 * +---------------------------------+ |     struct seg      +------------->+
 * segment data           | |                     |
 * +---------------------------------+ DRAM +----------+----------+ +---->+ | |
 * |     +---------------------------------+
 * +---------------------------------+                     v                   |
 * +->+                                 | |          segment data           |
 * +----------+----------+        |  |  +---------------------------------+
 * +---------------------------------+          |                     |        |
 * |  |                                 | |                                 |
 * +--+     struct seg      |        |  |  +---------------------------------+
 * +---------------------------------+       |  |                     |        |
 * |  |                                 | |                                 | |
 * +----------+----------+        |  |  +---------------------------------+
 * +---------------------------------+       |             |                   |
 * |  |                                 | | +<------+             v |  |
 * +---------------------------------+
 * +---------------------------------+          +----------+----------+        |
 * |  |                                 | |                     |        |  |
 * +---------------------------------+ |     struct seg      +--------+  |  | |
 *                                              |                     | |
 * +---------------------------------+
 *                                              +----------+----------+ |  | |
 *                                                         | |
 * +---------------------------------+ v                      |  | |
 *                                              +----------+----------+ |
 * +---------------------------------+ |                     |           |  | |
 *                                              |     struct seg +-----------+
 * +---------------------------------+ |                     |              | |
 *                                              +---------------------+
 * +---------------------------------+
 *
 */


/* TODO(jason): make sure it is less than one cacheline */
struct seg {
    TAILQ_ENTRY(seg) seg_tqe;
    uint32_t seg_id; /* the segment id in segment table,
                      * use seg_id instead of uint8_t*
                      * because seg address change after restart,
                      * and this also saves four byte for each seg
                      *
                      * maybe we can drop this as well since it can be
                      * calculated using address between datapool_base
                      * */

    uint32_t write_offset; /* used to calculate the write pos */
    uint32_t occupied_size; /* used size, less than seg_size because of
                             * internal fragmentation and update/deletion */

    proc_time_i create_at;
    delta_time_i ttl;
    uint32_t n_hit; /* only update when the seg is sealed */
    uint32_t n_hit_last; /* number of hits in the last window */

    uint32_t n_item; /* the number of usable items
                      * TODO (jason): could remove this field */
    uint16_t refcount; /* # items that can't be evicted */
    uint8_t locked; /* whether the seg is locked for eviction, used 1 byte
                     * because we need atomic operation on it,
                     * we can reuse refcount for this purpose by setting it
                     * to negative val */
    uint8_t sealed : 1; /* whether it is full and no longer write to */
    uint8_t in_pmem : 1; /* whether the seg is in PMem, not used */
    uint8_t initialized : 1; /* is seg initialized */
    uint8_t recovered : 1; /* whether the items on this seg have been
                              recovered*/

    uint16_t unused; /* unused, must be 0 */
};

TAILQ_HEAD(seg_tqh, seg);
TAILQ_ENTRY(seg) seg_tqe;

/* the order of field is optimized for CPU cacheline,
 * if it is using DRAM only, all frequent accessed field must not exceed 64 B
 * we may want to merge this header with datapool header */
struct seg_heapinfo {
    /* the first max_nseg_dram points to the segments in DRAM,
     * the next max_nseg_pmem points the segments in PMem */
    struct seg *segs;           /* seg headers, note that this is not part of heap_base allocated memory */
    size_t seg_size;
    uint32_t concat_seg : 1;
    uint32_t prealloc : 1;
    uint32_t prefault : 1;
    /* seg score priority queue */

    /* dram related */
    uint8_t *base_dram; /* address where seg data starts */
    uint32_t nseg_dram; /* # seg allocated */
    uint32_t max_nseg_dram; /* max # seg allowed */
    size_t size_dram;
    char *poolpath_dram;
    char *poolname_dram;
    struct datapool *pool_dram;

    /* pmem related */
    uint8_t *base_pmem; /* address where seg data starts */
    uint32_t nseg_pmem; /* # seg allocated */
    uint32_t max_nseg_pmem; /* max # seg allowed */
    size_t size_pmem;
    char *poolpath_pmem;
    char *poolname_pmem;
    struct datapool *pool_pmem;

//    struct seg *persisted_seg_hdr; /* persisted copy of seg headers
//                                    * once we fix datapool, we need to
//                                    * add a magic after persisted headers
//                                    * to avoid unnoticed data corruption */
    uint8_t *reserved_seg; /* reserved for DRAM/PMem migration and recovery */
    //    time_t time_started;
};

extern struct seg_heapinfo heap;


#define SEG_SIZE                MiB
#define SEG_MEM                 (64 * MiB)
#define SEG_PREALLOC            true
#define SEG_EVICT_OPT           EVICT_CTE
#define SEG_USE_CAS             true
#define ITEM_SIZE_MAX           (SEG_SIZE - ITEM_HDR_SIZE)
#define HASH_POWER              20
#define SEG_DATAPOOL            NULL
#define SEG_DATAPOOL_PREFAULT   false
#define SEG_DATAPOOL_NAME_DRAM       "seg_datapool_dram"
#define SEG_DATAPOOL_NAME_PMEM       "seg_datapool_pmem"

/*          name                    type                default              description */
#define SEG_OPTION(ACTION)                                                                                         \
    ACTION( seg_size,              OPTION_TYPE_UINT,   SEG_SIZE,              "Segment size"                      )\
    ACTION( seg_mem_dram,          OPTION_TYPE_UINT,   SEG_MEM,               "Max memory used for DRAM caching (byte)")\
    ACTION( seg_mem_pmem,          OPTION_TYPE_UINT,   0,                     "Max memory used for PMem caching (byte)")\
    ACTION( seg_prealloc,          OPTION_TYPE_BOOL,   SEG_PREALLOC,          "Pre-allocate segs at setup"        )\
    ACTION( seg_evict_opt,         OPTION_TYPE_UINT,   SEG_EVICT_OPT,         "Eviction strategy"                 )\
    ACTION( seg_item_use_cas,      OPTION_TYPE_BOOL,   SEG_USE_CAS,           "Store CAS value in item"           )\
    ACTION( seg_hash_power,        OPTION_TYPE_UINT,   HASH_POWER,            "Power for lookup hash table"       )\
    ACTION( datapool_path_dram,    OPTION_TYPE_STR,    SEG_DATAPOOL,          "Path to DRAM data pool"                 )\
    ACTION( datapool_name_dram,    OPTION_TYPE_STR,    SEG_DATAPOOL_NAME_DRAM,"Seg DRAM data pool name"                )\
    ACTION( datapool_path_pmem,    OPTION_TYPE_STR,    SEG_DATAPOOL,          "Path to PMem data pool"                 )\
    ACTION( datapool_name_pmem,    OPTION_TYPE_STR,    SEG_DATAPOOL_NAME_PMEM,"Seg PMem data pool name"                )\
    ACTION( prefault_pmem,         OPTION_TYPE_BOOL,   SEG_DATAPOOL_PREFAULT, "Prefault Pmem"                )

typedef struct {
    SEG_OPTION(OPTION_DECLARE)
} seg_options_st;

#include <cc_define.h>
#include <cc_metric.h>


/*          name                    type            description */
#define SEG_METRIC(ACTION)                                                               \
    ACTION( seg_req,               METRIC_COUNTER, "# req for new seg"                  )\
    ACTION( seg_req_ex,            METRIC_COUNTER, "# seg get exceptions"               )\
    ACTION( seg_evict,             METRIC_COUNTER, "# segs evicted"                     )\
    ACTION( seg_evict_ex,             METRIC_COUNTER, "# segs evict exceptions"                     )\
    ACTION( seg_expire,            METRIC_COUNTER, "# segs removed due to expiration"   )\
    ACTION( seg_curr_dram,         METRIC_GAUGE,   "# currently active segs in DRAM"    )\
    ACTION( seg_curr_pmem,         METRIC_GAUGE,   "# currently active segs in PMem"    )\
    ACTION( item_curr,             METRIC_GAUGE,   "# current items"                    )\
    ACTION( item_curr_bytes,       METRIC_GAUGE,   "# used bytes including item header" )\
    ACTION( item_alloc,            METRIC_COUNTER, "# items allocated"                  )\
    ACTION( item_alloc_ex,         METRIC_COUNTER, "# item alloc errors"                )\
    ACTION( hash_lookup,           METRIC_COUNTER, "# of hash lookups"                  )\
    ACTION( hash_insert,           METRIC_COUNTER, "# of hash inserts"                  )\
    ACTION( hash_remove,           METRIC_COUNTER, "# of hash deletes"                  )\
    ACTION( hash_traverse,         METRIC_COUNTER, "# of nodes touched"                 )

typedef struct {
    SEG_METRIC(METRIC_DECLARE)
} seg_metrics_st;

/*          name                type            description */
#define PERTTL_METRIC(ACTION)                                                        \
    ACTION( item_curr,          METRIC_GAUGE,   "# items stored"                    )\
    ACTION( item_update,        METRIC_GAUGE,   "# holes caused by updates"         )\
    ACTION( item_del,           METRIC_GAUGE,   "# holes caused by deletion"        )\
    ACTION( item_curr_bytes,    METRIC_GAUGE,   "size of items stored"              )\
    ACTION( item_update_bytes,  METRIC_GAUGE,   "size of holes caused by updates"   )\
    ACTION( item_del_bytes,     METRIC_GAUGE,   "size of holes caused by deletion"  )\
    ACTION( seg_curr,           METRIC_GAUGE,   "# segs"                            )\

typedef struct {
    PERTTL_METRIC(METRIC_DECLARE)
} seg_perttl_metrics_st;


#define PERTTL_INCR(idx, metric) INCR(&perttl[idx], metric)
#define PERTTL_DECR(idx, metric) DECR(&perttl[idx], metric)
#define PERTTL_INCR_N(idx, metric, delta) INCR_N(&perttl[idx], metric, delta)
#define PERTTL_DECR_N(idx, metric, delta) DECR_N(&perttl[idx], metric, delta)


static inline struct seg *
item_to_seg(struct item *it)
{
    return &heap.segs[it->seg_id];
}

static inline bool
seg_is_locked(struct seg *seg)
{
    return __atomic_load_n(&seg->locked, __ATOMIC_RELAXED) > 0;
}


static inline bool
seg_ref(struct seg *seg)
{
    if (!seg_is_locked(seg)){
        /* this does not strictly remove race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

static inline void
seg_deref(struct seg *seg)
{
    ASSERT(seg->refcount > 0);

    seg->refcount--;
}

static inline uint8_t *
seg_get_data_start(uint32_t seg_id)
{
    if (seg_id >= heap.max_nseg_dram) {
        return heap.base_pmem + heap.seg_size * (seg_id - heap.max_nseg_dram);
    } else {
        return heap.base_dram + heap.seg_size * seg_id;
    }
}

static inline bool
seg_use_dram(void)
{
    return heap.max_nseg_dram > 0;
}

static inline bool
seg_use_pmem(void)
{
    return heap.max_nseg_pmem > 0;
}


void seg_setup(seg_options_st *options, seg_metrics_st *metrics);

void seg_teardown(void);

struct seg *seg_get_new(void);

/*
 * remove all items on this segment
 * make sure segment is locked and ref_cnt 0
 * indicating no other threads are accessing items on the seg
 */
bool seg_rm_all_item(uint32_t seg_id);

void seg_rm_expired_seg(uint32_t seg_id);

void _seg_print(uint32_t seg_id);
