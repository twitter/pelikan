#pragma once

#include "datapool/datapool.h"
#include "item.h"
#include "segevict.h"

#include <cc_define.h>
#include <cc_itt.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_util.h>
#include <time/cc_timer.h>
#include <time/cc_wheel.h>


#include <pthread.h>
#include <stdbool.h>
#include <stddef.h>


/**
 * The cache space is divided into fix-sized segments.
 * A segment is either linked in a global free pool, or
 * linked in TTL-indexed segment list.
 *
 * ALL segments in the TTL-indexed segment list have similar TTLs and pointers
 * to the head and tail of each segment list are stored in TTL bucket.
 *
 * struct seg contains the metadata of each segment,
 * the start of segment data can be calculated from the segment id because
 * all cache space is preallocated and segment is of fixed size.
 *
 * note that although segment header is always kept in DRAM,
 * the actual segment data can be in other storage media, such as PMem.
 * Storing the seg_header in DRAM requires
 *
 * see the following for more details
 *
 * Juncheng Yang, Yao Yue, Rashmi Vinayak.
 * Segcache: a memory-efficient and highly-scalable DRAM cache for small objects. NSDI'21.
 *
 **/


struct seg {
    int32_t         seg_id;

    int32_t         write_offset;  /* current write pos */
    int32_t         occupied_size; /* the number of live bytes,
                                    * smaller than seg_size due to
                                    * internal fragmentation and
                                    * item update/deletion */

    int32_t         n_item;        /* # live items in the segment */
    int32_t         prev_seg_id;   /* prev seg in ttl_bucket or free pool */
    int32_t         next_seg_id;   /* next seg in ttl_bucket or free pool */

    int16_t         w_refcount;    /* # concurrent reads, >0 means the seg
                                    * cannot be evicted */
    int16_t         r_refcount;    /* # concurrent writes, >0 means the seg
                                    * cannot be evicted */

    int32_t         n_hit;         /* only update when the seg is sealed */
    int32_t         n_active;
    int32_t         n_active_byte;

    proc_time_i     create_at;
    delta_time_i    ttl;
    proc_time_i     merge_at;

    uint8_t         accessible;    /* indicate the seg is being evicted */
    uint8_t         evictable;     /* not evictable when it is evicted by
                                    * other thread, or it is expired */

    uint8_t         recovered : 1; /* whether the items on this seg have been
                                    * recovered */

    uint16_t        unused;       /* unused, must be 0 */
};


/**
 * the order of field is optimized for CPU cacheline
 **/
struct seg_heapinfo {
    struct seg          *segs;          /* seg headers */
    size_t              seg_size;

    uint8_t             *base;          /* address where seg data starts */
    int32_t             n_free_seg;     /* # seg allocated */
    int32_t             max_nseg;       /* max # seg allowed */
    size_t              heap_size;

    int32_t             free_seg_id;    /* this is the head of free pool */

    char                *poolpath;
    char                *poolname;
    struct datapool     *pool;

    uint32_t            prealloc : 1;
    uint32_t            prefault : 1;

    int32_t             n_reserved_seg;

    pthread_mutex_t     mtx;

    proc_time_i         time_started;
};


enum seg_state_change {
    SEG_ALLOCATION = 0,
    SEG_CONCURRENT_GET,   /* multiple threads can concurrently get segs when
                             * the last seg is full, but only one thread will
                             * success, the other threads will return the
                             * seg to free pool */
    SEG_EVICTION,
    SEG_EXPIRATION,

    SEG_INVALID_CHANGE,
};


#define SEG_SIZE MiB
#define SEG_MEM (64 * MiB)
#define SEG_PREALLOC true
#define SEG_EVICT_OPT EVICT_MERGE_FIFO
#define SEG_USE_CAS true
#define ITEM_SIZE_MAX (SEG_SIZE - ITEM_HDR_SIZE)
#define HASH_POWER 16
#define N_THREAD 1
#define SEG_DATAPOOL NULL
#define SEG_DATAPOOL_PREFAULT true
#define SEG_DATAPOOL_NAME "seg_datapool"

#define SEG_MATURE_TIME 20
#define SEG_N_MAX_MERGE 8
#define SEG_N_MERGE     4


/*          name                    type            default                 description */
#define SEG_OPTION(ACTION)                                                                                                                                                               \
    ACTION(seg_size,            OPTION_TYPE_UINT,   SEG_SIZE,               "Segment size"                                                                                              )\
    ACTION(heap_mem,            OPTION_TYPE_UINT,   SEG_MEM,                "Max memory used for caching (byte)"                                                                        )\
    ACTION(seg_prealloc,        OPTION_TYPE_BOOL,   SEG_PREALLOC,           "Pre-allocate segs at setup"                                                                                )\
    ACTION(seg_evict_opt,       OPTION_TYPE_UINT,   SEG_EVICT_OPT,          "Eviction strategy (0: no eviction, 1: random, 2: FIFO, 3: close to expire, 4: utilization, 5: merge fifo"  )\
    ACTION(seg_use_cas,         OPTION_TYPE_BOOL,   SEG_USE_CAS,            "whether use cas, should be true"                                                                           )\
    ACTION(seg_mature_time,     OPTION_TYPE_UINT,   SEG_MATURE_TIME,        "min time before a segment can be considered for eviction"                                                  )\
    ACTION(seg_n_max_merge,     OPTION_TYPE_UINT,   SEG_N_MAX_MERGE,        "max number of segments can be evicted/merged in one eviction"                                              )\
    ACTION(seg_n_merge,         OPTION_TYPE_UINT,   SEG_N_MERGE,            "the target number of segment to be evicted/merge in one eviction"                                          )\
    ACTION(hash_power,          OPTION_TYPE_UINT,   HASH_POWER,             "Power for lookup hash table"                                                                               )\
    ACTION(seg_n_thread,        OPTION_TYPE_UINT,   N_THREAD,               "number of threads"                                                                                         )\
    ACTION(datapool_path,       OPTION_TYPE_STR,    SEG_DATAPOOL,           "Path to DRAM data pool"                                                                                    )\
    ACTION(datapool_name,       OPTION_TYPE_STR,    SEG_DATAPOOL_NAME,      "Seg DRAM data pool name"                                                                                   )\
    ACTION(datapool_prefault,   OPTION_TYPE_BOOL,   SEG_DATAPOOL_PREFAULT,  "Prefault Pmem"                                                                                             )

typedef struct {
    SEG_OPTION(OPTION_DECLARE)
} seg_options_st;


/*          name                    type            description */
#define SEG_METRIC(ACTION)                                                                   \
    ACTION(seg_get,             METRIC_COUNTER,     "# req for new seg"                     )\
    ACTION(seg_get_ex,          METRIC_COUNTER,     "# seg get exceptions"                  )\
    ACTION(seg_return,          METRIC_COUNTER,     "# segment returns"                     )\
    ACTION(seg_evict,           METRIC_COUNTER,     "# segs evicted"                        )\
    ACTION(seg_evict_retry,     METRIC_COUNTER,     "# retried seg eviction"                )\
    ACTION(seg_evict_ex,        METRIC_COUNTER,     "# segs evict exceptions"               )\
    ACTION(seg_expire,          METRIC_COUNTER,     "# segs removed due to expiration"      )\
    ACTION(seg_merge,           METRIC_GAUGE,       "# seg merge"                           )\
    ACTION(seg_curr,            METRIC_GAUGE,       "# active segs"                         )\
    ACTION(item_curr,           METRIC_GAUGE,       "# current items"                       )\
    ACTION(item_curr_bytes,     METRIC_GAUGE,       "# used bytes including item header"    )\
    ACTION(item_alloc,          METRIC_COUNTER,     "# items allocated"                     )\
    ACTION(item_alloc_ex,       METRIC_COUNTER,     "# item alloc errors"                   )\
    ACTION(hash_lookup,         METRIC_COUNTER,     "# hash lookups"                        )\
    ACTION(hash_insert,         METRIC_COUNTER,     "# hash inserts"                        )\
    ACTION(hash_remove,         METRIC_COUNTER,     "# hash deletes"                        )\
    ACTION(hash_bucket_alloc,   METRIC_COUNTER,     "# overflown hash bucket allocations"   )\
    ACTION(hash_tag_collision,  METRIC_COUNTER,     "# tag collision"                       )

typedef struct {
    SEG_METRIC(METRIC_DECLARE)
} seg_metrics_st;

/*          name                type            description */
#define PERTTL_METRIC(ACTION)                                                    \
    ACTION(item_curr,           METRIC_GAUGE, "# items stored"                  )\
    ACTION(item_update,         METRIC_GAUGE, "# holes caused by updates"       )\
    ACTION(item_del,            METRIC_GAUGE, "# holes caused by deletion"      )\
    ACTION(item_curr_bytes,     METRIC_GAUGE, "size of items stored"            )\
    ACTION(item_update_bytes,   METRIC_GAUGE, "size of holes caused by updates" )\
    ACTION(item_del_bytes,      METRIC_GAUGE, "size of holes caused by deletion")\
    ACTION(seg_curr,            METRIC_GAUGE, "# segs"                          )

typedef struct {
    PERTTL_METRIC(METRIC_DECLARE)
} seg_perttl_metrics_st;


#define PERTTL_INCR(idx, metric) INCR(&perttl[idx], metric)
#define PERTTL_DECR(idx, metric) DECR(&perttl[idx], metric)
#define PERTTL_INCR_N(idx, metric, delta) INCR_N(&perttl[idx], metric, delta)
#define PERTTL_DECR_N(idx, metric, delta) DECR_N(&perttl[idx], metric, delta)

extern struct seg_heapinfo heap; /* info of all allocated segs */

void
seg_setup(seg_options_st *options, seg_metrics_st *metrics);

void
seg_teardown(void);

/**
 * get a new segment for writing data, if there is no new segment,
 * segment eviction kicks in first then return evicted segment
 *
 * @return id of the new segment
 */
int32_t
seg_get_new(void);

/**
 * add the seg to free pool, the seg can be allocated (during setup) or
 * evicted during concurrent evictions.
 *
 * concurrent evictions can happen because of optimistic concurrency control,
 * concurrent evictions evict objects before needed to do, but
 * concurrent evictions happens when data insertion rate is high, thus
 * it makes sense to eivct concurrently
 *
 * because only one segment will be linked to ttl_bucket successfully,
 * the rest of evicted segments will return to free pool
 *
 * @param seg_id id of the segment
 * @param segment state change reason
 * */
void
seg_add_to_freepool(int32_t seg_id, enum seg_state_change reason);


/**
 * remove all items on this segment
 * @param seg_id id of the segment
 * @param reason why do we remove all objects from this seg
 *
 * @return true if current thread is able to grab the lock to
 * remove all items on the segment, otherwise false
 */
bool
rm_all_item_on_seg(int32_t seg_id, enum seg_state_change reason);

/**
 * remove all objects on this segment because it is expired
 *
 * @param seg_id id of the segment
 * @return CC_OK if the thread is able to expire the seg
 */
rstatus_i
expire_seg(int32_t seg_id);


/**
 * because a segment can be locked for expiration or eviction,
 * during which data cannot be read
 * this return whether the segment is still available to read
 *
 * @param seg_id
 * @return true if we can still read data from the seg
 */
bool
seg_is_accessible(int32_t seg_id);


/**
 * increase write ref_counter on the segment, a segment being actively
 * written to cannot be expired/evicted
 */
bool
seg_w_ref(int32_t seg_id);

/**
 * decrease write ref_counter on the segment
 */
void
seg_w_deref(int32_t seg_id);


/* get the data start of the segment */
static inline uint8_t *
get_seg_data_start(int32_t seg_id)
{
    return heap.base + heap.seg_size * seg_id;
}


void
seg_init(int32_t seg_id);


/**
 * get a seg from free pool,
 *
 * use_reserved: merge-based eviction reserves one seg per thread
 * return the segment id if there are free segment, -1 if not
 */
int32_t
seg_get_from_freepool(bool use_reserved);

/**
 * wait until no other threads are accessing the seg (refcount == 0)
 */
void
seg_wait_refcnt(int32_t seg_id);

/**
 * remove the segment from the TTL bucket and segment chain
 */
void
rm_seg_from_ttl_bucket(int32_t seg_id);


#define SEG_PRINT(id, msg, log) do {                                            \
        log("%s, seg %d seg size %zu, create_at time %d, merge at %d, age %d"   \
        ", ttl %d, evictable %u, accessible %u"                                 \
        ", write offset %d, occupied size %d"                                   \
        ", %d items, n_hit %d, read refcount %d, write refcount %d"             \
        ", prev_seg %d, next_seg %d",                                           \
        msg, id, heap.seg_size, heap.segs[id].create_at, heap.segs[id].merge_at,\
        heap.segs[id].merge_at > 0 ?                                            \
        time_proc_sec() - heap.segs[id].merge_at :                              \
        time_proc_sec() - heap.segs[id].create_at, heap.segs[id].ttl,           \
        __atomic_load_n(&(heap.segs[id].evictable), __ATOMIC_RELAXED),          \
        __atomic_load_n(&(heap.segs[id].accessible), __ATOMIC_RELAXED),         \
        __atomic_load_n(&(heap.segs[id].write_offset), __ATOMIC_RELAXED),       \
        __atomic_load_n(&(heap.segs[id].occupied_size), __ATOMIC_RELAXED),      \
        __atomic_load_n(&(heap.segs[id].n_item), __ATOMIC_RELAXED),             \
        __atomic_load_n(&(heap.segs[id].n_hit), __ATOMIC_RELAXED),              \
        __atomic_load_n(&(heap.segs[id].r_refcount), __ATOMIC_RELAXED),         \
        __atomic_load_n(&(heap.segs[id].w_refcount), __ATOMIC_RELAXED),         \
        heap.segs[id].prev_seg_id, heap.segs[id].next_seg_id);                  \
    } while (0)

