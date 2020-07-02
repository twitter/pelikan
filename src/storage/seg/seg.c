#include "seg.h"
#include "constant.h"
#include "datapool/datapool.h"
#include "hashtable.h"
#include "item.h"
#include "segevict.h"
#include "ttlbucket.h"

#include <cc_mm.h>
#include <cc_util.h>

#include <errno.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

#define SEG_MODULE_NAME "storage::seg"
#define TRIES_MAX 10

extern struct setting setting;

struct seg_heapinfo heap; /* info of all allocated segs */
struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];
static uint32_t hash_power = HASH_POWER;
struct hash_table *hash_table = NULL;
struct locktable cas_table;


static bool seg_initialized = false;
seg_metrics_st *seg_metrics = NULL;
seg_options_st *seg_options = NULL;
seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];

proc_time_i flush_at = -1;
bool use_cas = false;


void
_seg_print(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    log_debug("seg id %" PRIu32 " seg size %zu, create_at time %" PRId32
         ", ttl %" PRId32 ", initialized %u, "
         "%" PRIu32 " items, write offset %" PRIu32 ", occupied size "
         "%" PRIu32 ", n_hit %" PRIu32 ", n_hit_last %" PRIu32 ", read "
         "refcount "
         "%" PRIu32 ", write refcount %" PRIu32,
            seg->seg_id, heap.seg_size, seg->create_at, seg->ttl,
            seg->initialized, seg->n_item, seg->write_offset,
            seg->occupied_size, seg->n_hit, seg->n_hit_last,
            __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED));
}

static inline void
_debug_print_seg_list(void)
{
    log_debug("free seg %d\n", heap.free_seg_id);
    for (int i = 0; i < MAX_TTL_BUCKET; i++) {
        struct ttl_bucket *ttl_bucket = &ttl_buckets[i];
        if (ttl_bucket->first_seg_id == -1)
            continue;
        log_debug("ttl bucket %d first seg %d last seg %d\n", i,
                ttl_bucket->first_seg_id, ttl_bucket->last_seg_id);
    }
    for (int i = 0; i < heap.max_nseg; i++)
        log_debug("seg %d: prev %d next %d\n", i, heap.segs[i].prev_seg_id,
                heap.segs[i].next_seg_id);
}

/* when we want to evict/remove a seg, we need to make sure no other
 * threads are still reading from the seg, so
 * 1. we locked the seg to prevent future access,
 * 2. perform non-destructive eviction op (such as remove hashtable entries)
 * 3. then we check and wait until refcount becomes 0
 * 4. init the seg for future use */
static inline bool
_seg_lock(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    return __atomic_exchange_n(&seg->locked, 1, __ATOMIC_RELAXED) == 0 ? true :
                                                                         false;
}

/*
 * wait until the seg is available to be freed (refcount == 0)
 */
static inline void
_seg_wait_refcnt(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    ASSERT(seg->locked != 0);

    log_verb("wait for seg %" PRIu32 " refcount, current read refcount %" PRIu32
             ", write refcount %" PRIu32,
            seg_id, __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED));

    /* TODO (jason): a better way to spin lock here? */
    while (__atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED) > 0) {
        ;
    }

    while (__atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED) > 0) {
        ;
    }

    log_verb("wait for seg %" PRIu32 " refcount finishes", seg_id);
}

static inline bool
_seg_heap_full(void)
{
    return __atomic_load_n(&heap.nseg, __ATOMIC_RELAXED) >= heap.max_nseg;
}


static inline void
_sync_seg_hdr(void)
{
    log_crit("wait for impl :(");
}


#ifdef do_not_define
/*
 * Recreate items from a persisted segment, used when shared memory or
 * external storage is enabled
 * new_seg points to a dynamically allocated in-DRAM data structure that
 * holds the current segment, we copy the valid item from seg_old to
 */
static void
_seg_recreate_items(uint32_t seg_id, struct seg *new_seg)
{
    struct item *it;
    uint32_t i;

    /* we copy the seg header from shared memory/PMem/external storage
     * to avoid repeated small read from PMem/external storage */
    struct seg *seg_old_p =
            (struct seg *)(heap.base + sizeof(struct seg) * seg_id);
    struct seg seg_old = *seg_old_p;
    uint8_t *data_start = seg_old.data_start;

    //  p = &segclass[seg->id];
    //  p->nfree_item = p->nitem;
    for (i = 0; i < p->nitem; i++) {
        it = _seg_to_item(seg, i, p->size);
        if (it->is_linked) {
            p->next_item_in_seg = (struct item *)&seg->data[0];
            INCR(seg_metrics, item_curr);
            INCR(seg_metrics, item_alloc);
            PERSEG_INCR(seg->id, item_curr);
            item_relink(it);
            if (--p->nfree_item != 0) {
                p->next_item_in_seg =
                        (struct item *)((char *)p->next_item_in_seg + p->size);
            } else {
                p->next_item_in_seg = NULL;
            }
        } else if (it->in_freeq) {
            _seg_put_item_into_freeq(it, seg->id);
        } else if (it->klen && !_seg_check_no_refcount(seg)) {
            /* before reset, item could be only reserved
             * ensure that seg has a reserved item(s)
             */
            item_release(&it);
        }
    }
}


/* given the new mapping address, recover the memory address in seg headers */
static rstatus_i
_seg_recover_seg_header(void)
{
    uint32_t i;

    for (i = 0; i < heap.max_nseg; i++) {
        heap.persisted_seg_hdr[i].
    }
}

/* recover the items on the old segment,
 * this comes with compaction, meaning deleted item and expired item are not
 * recovered
 */
static rstatus_i
_seg_recover_one_seg(uint32_t old_seg_id)
{
    uint16_t ttl_bucket_idx;
    struct ttl_bucket *ttl_bucket;
    uint8_t *curr, *tmp_seg_data = NULL;
    struct item *oit, *nit;
    struct seg *seg; /* the seg we write re-created item to */
    struct seg *old_segs = heap.persisted_seg_hdr;
    struct seg *old_seg = &heap.persisted_seg_hdr[old_seg_id];
    uint8_t *old_seg_data;
    if (old_seg_id >= heap.max_nseg) {
        old_seg_data =
                heap.base_pmem + heap.seg_size * (old_seg_id - heap.max_nseg);
    } else {
        old_seg_data = heap.base + heap.seg_size * old_seg_id;
    }

    /* we may use this segment for newly rewritten data, so we make a copy
     * of the segment data before we recover
     */
    tmp_seg_data = cc_zalloc(heap.seg_size);
    cc_memcpy(tmp_seg_data, old_seg_data, heap.seg_size);
    curr = tmp_seg_data;

    /* clear the old seg */

    while (curr < tmp_seg_data + heap.seg_size) {
#    if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(*(uint64_t *)curr == SEG_MAGIC);
        curr += 8;
#    endif
        oit = (struct item *)curr;
        curr += item_ntotal(oit);
        if (!oit->valid) {
            continue;
        }

        if (old_seg_id != oit->seg_id) {
            log_warn("detect seg_id inconsistency in seg_recovery\n");
            return CC_ERROR;
        }

        if (old_seg->create_at + old_seg->ttl < time_proc_sec()) {
            /* expired */
            continue;
        }

        /* find the segment where we can write */
        nit = ttl_bucket_reserve_item(
                old_seg->ttl, item_ntotal(oit), false, NULL, NULL);
        if (nit == NULL) {
            /* if current ttl_bucket does not have a segment allocated
             * or the allocated seg is full, we will reuse this old_sg */


            new_seg->ttl = ttl;
            TAILQ_INSERT_TAIL(&ttl_bucket->seg_q, new_seg, seg_tqe);
            curr_seg->sealed = 1;

            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            seg_data_start = seg_get_data_start(curr_seg->seg_id);
            offset = __atomic_fetch_add(
                    &(curr_seg->write_offset), sz, __ATOMIC_RELAXED);

            uint32_t occupied_size = __atomic_add_fetch(
                    &(curr_seg->occupied_size), sz, __ATOMIC_RELAXED);
            ASSERT(occupied_size <= heap.seg_size);

            ASSERT(seg_data_start != NULL);
            it = (struct item *)(seg_data_start + offset);
            if (seg_p) {
                *seg_p = curr_seg;
            }

            PERTTL_INCR(ttl_bucket_idx, item_curr);
            PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);
        }

        ttl_bucket_idx = find_ttl_bucket_idx(old_seg->ttl);
        ttl_bucket = &ttl_buckets[ttl_bucket_idx];
        seg = TAILQ_LAST(&ttl_bucket->seg_q, seg_tqh);
        bool reuse_old_seg = false;
        if (seg == NULL) {
            reuse_old_seg = true;
        } else {
            uint8_t *seg_data_start = seg_get_data_start(seg->seg_id);
            uint32_t offset = __atomic_fetch_add(
                    &(seg->write_offset), item_ntotal(oit), __ATOMIC_RELAXED);
        }

        new_seg->create_at = old_seg->create_at;

        struct seg *curr_seg, *new_seg;


        /* recreate item on the heap */
        item_recreate(&nit, oit, old_seg->ttl, old_seg->create_at);
        /* insert into hash table */
        key = (struct bstring){nit->klen, item_key(nit)};
        item_insert(nit, &key);
    }

    qsort()
}

/*
 * Recreate segs structure when persistent memory/external features are enabled
 * first we build expiration tree and segment score tree
 * second we check whether there are any expired segment
 * third, we start with the least occupied seg, recreate items in the seg
 *
 * NOTE: we might lose one (or more) segments of items if the heap is full
 *
 * time_since_create: time since the datapool is created
 */
static rstatus_i
_seg_recovery(uint8_t *base, uint32_t max_nseg)
{
    uint32_t i;
    struct bstring key;
    uint8_t *curr;
    struct seg *new_seg, *old_seg;
    struct seg *old_segs = heap.persisted_seg_hdr;
    struct item *oit, *nit; /* item on the old segment and recreated item */
    uint32_t n_seg_to_recover = max_nseg;

    /* we need to update old_set ttl first */
    /* TODO(jason): ASSUME we have updated time_started and proc_sec when
     * loading datapool */


    /* copy the seg to DRAM, then scan the copied seg
     *
     * when DRAM+PMem tiered storage is used,
     * we discard all objects in DRAM
     * this may or may not be wise decision, but we do this for now
     *
     * */

    /* we need to allocate new seg from start of the datapool
     * so we don't need to explicitly change ttl_bucket_reserve_item
     * but then we need to backup the overwritten data */
    delta_time_i create_at_earliest;
    uint32_t earliest_seg_id = UINT32_MAX;
    while (n_seg_to_recover > 0) {
        /* TODO (jason) currently a O(N^2) metadata scanning,
         * might want to change to O(NlogN), but given this is one-time
         * initialization and the number of segments are limited,
         * keep this for now
         */
        create_at_earliest = time_proc_sec() + 1;
        earliest_seg_id = UINT32_MAX;
        curr = base;
        /* find the seg with the earliest creation time */
        for (i = 0; i < max_nseg; i++) {
            if (old_segs[i].recovered == 1) {
                continue;
            }
            oit = (struct item *)curr;
            if (old_segs[i].create_at < create_at_earliest) {
                create_at_earliest = old_segs[i].create_at;
                earliest_seg_id = i;
            }
            curr += heap.seg_size;
        }
        _seg_recover_one_seg(earliest_seg_id);
        old_segs[earliest_seg_id].recovered = 1;
        n_seg_to_recover -= 1;
    }

    /* all segs have been recovered, hashtable has been rebuilt */


    return CC_OK;
}
#endif

static void
_seg_init(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    uint8_t *data_start = seg_get_data_start(seg_id);

    cc_memset(seg, 0, sizeof(*seg));
    /* TODO (jason): at eviction/expiration we clear the seg data twice */
    cc_memset(data_start, 0, heap.seg_size);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    *(uint64_t *)(data_start) = SEG_MAGIC;
    seg->write_offset = 8;
    seg->occupied_size = 8;
#endif

    seg->seg_id = seg_id;
    seg->prev_seg_id = -1;
    seg->next_seg_id = -1;
    seg->initialized = 1;
    seg->create_at = time_proc_sec();
    seg->locked = 0;
}

/*
 * remove all items on this segment,
 * most of the time (common case), the seg should have no writers because
 * the eviction algorithms will try to pick the segment with no writer
 *
 * However, it is possible we are evicting a segment with writer in the
 * following cases:
 * 1. it takes too long (compared to its TTL) for the segment to
 *      finish writing and it has expired
 * 2. cache size is too small and the workload uses too many ttl buckets
 *
 *
 * indicating the seg hsa no writer, but can have readers, doing so allows us
 * to avoid locking to access the metadata of the seg and
 * avoid using refcount for writer
 *
 * the only time when eviction happens and segment is not sealed is that
 * the segment TTL is too short or writing is too slow so that finish writing
 * takes longer than TTL, in this case, we use optimistic design and assume that
 * there are not too many concurrent writers, if we detect there are new writes
 * after we have removed all entries from hashtable, we have to rm the
 * new entries again
 *
 * because multiple threads could try to evict the seg at the same time,
 * return true if current thread is able to wipe this seg, otherwise false
 */
bool
seg_rm_all_item(int32_t seg_id)
{
    /* lock the seg to prevent other threads evicting this segment
     * we do all the computation after lock the seg, this gives two benefits
     * 1. prevent possible race condition and
     * 2. wait for readers while doing useful work
     * */
    if (!_seg_lock(seg_id)) {
        /* fail to lock, some other thread is expiring/evicting this seg */
        log_warn("evict seg %" PRIu32 ": unable to lock", seg_id);
        INCR(seg_metrics, seg_evict_ex);

        return false;
    }

    struct seg *seg = &heap.segs[seg_id];
    struct item *it;
    int32_t ttl_n_seg;
    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    int32_t prev_seg_id = seg->prev_seg_id;
    int32_t next_seg_id = seg->next_seg_id;
    struct ttl_bucket *ttl_bucket = &ttl_buckets[find_ttl_bucket_idx(seg->ttl)];
    uint32_t offset = __atomic_load_n(&seg->write_offset, __ATOMIC_RELAXED);

    log_debug("evict seg %" PRIu32 ", read refcount %" PRIu32 ", write refcount "
             "%" PRIu32 ", write offset %" PRIu32 ", occupied size %" PRIu32,
            seg_id, __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->write_offset), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->occupied_size), __ATOMIC_RELAXED));

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    /* lock to remove the seg from the seg list in ttl bucket
     * this will prevent future writer write to this seg */
    pthread_mutex_lock(&heap.mtx);
    log_debug("evict seg %" PRIu32 " locked", seg_id);

    if (prev_seg_id == -1) {
        ASSERT(ttl_bucket->first_seg_id == seg_id);
        ttl_bucket->first_seg_id = next_seg_id;
    } else {
        heap.segs[prev_seg_id].next_seg_id = next_seg_id;
    }

    if (next_seg_id == -1) {
        ASSERT(ttl_bucket->last_seg_id == seg_id);
        ttl_bucket->last_seg_id = prev_seg_id;
    } else {
        heap.segs[next_seg_id].prev_seg_id = prev_seg_id;
    }

    pthread_mutex_unlock(&heap.mtx);
    log_debug("evict seg %" PRIu32 " unlocked", seg_id);

    ttl_n_seg = __atomic_sub_fetch(&ttl_bucket->n_seg, 1, __ATOMIC_RELAXED);

    ASSERT(ttl_n_seg >= 0);
    ASSERT(offset <= heap.seg_size);

    uint32_t n_deleted = 0;
    uint32_t n_item = __atomic_load_n(&seg->n_item, __ATOMIC_RELAXED);

    while (curr - seg_data < offset) {
        it = (struct item *)curr;
        curr += item_ntotal(it);
        if (item_delete_it(it)) {
            n_deleted += 1;
        }
    }

    ASSERT(n_deleted == n_item);
    /* all operation up till here does not require refcount to be 0
     * because the data on the segment is not cleared yet,
     * now we are ready to clear the segment data, we need to check refcount
     * since we have already locked the segment before removing entries
     * from hashtable, ideally by the time we have removed all hashtable
     * entries, all previous requests on this segment have all finished */
    _seg_wait_refcnt(seg_id);

    /* optimistic eviction:
     * because we didn't wait for refcount before remove hashtable entries
     * it is possible that there are some very slow writers, which finish
     * writing after we clear the hashtable entries,
     * so we need to double check, in most cases, this should not happen */

    if (__atomic_load_n(&seg->n_item, __ATOMIC_RELAXED) > 0) {
        INCR(seg_metrics, seg_evict_retry);
        /* because we don't know which item is newly written, so we
         * have to remove all items again */
        curr = seg_data;
        while (curr - seg_data < offset) {
            it = (struct item *)curr;
            curr += item_ntotal(it);
            if (item_delete_it(it)) {
                n_deleted += 1;
            }
        }
    }

    INCR(seg_metrics, seg_evict);

    log_debug("evict seg %" PRIu32 ": remove %" PRIu32 " items", seg_id,
            n_deleted);
    return true;
}

///* the segment points to by seg_id_pmem is empty and ready to use */
// static bool
// migrate_dram_to_pmem(uint32_t seg_id_dram, uint32_t seg_id_pmem)
//{
//    /* first thing, we lock the dram seg to prevent future access to it */
//    /* TODO(jason): change function signature to use struct seg instead of
//     * seg_id */
//
//    log_verb("migrate DRAM seg %" PRIu32 " to PMem seg %" PRIu32, seg_id_dram,
//            seg_id_pmem);
//
//    if (!_seg_lock(seg_id_dram)) {
//        return false;
//    }
//
//    struct item *oit, *nit;
//    struct seg *seg_dram = &heap.segs[seg_id_dram];
//    struct seg *seg_pmem = &heap.segs[seg_id_pmem];
//    uint8_t *seg_dram_data = seg_get_data_start(seg_id_dram);
//    uint8_t *seg_pmem_data = seg_get_data_start(seg_id_pmem);
//
//    cc_memcpy(seg_dram, seg_pmem, sizeof(struct seg));
//    cc_memcpy(seg_pmem_data, seg_dram_data, heap.seg_size);
//
//    seg_pmem->refcount = 0;
//    seg_pmem->locked = 0;
//    seg_pmem->seg_id = seg_id_pmem;
//    seg_pmem->in_pmem = 1;
//
//    /* relink hash table, this needs to be thread-safe
//     * we don't need lock here,
//     * since we require hashtable update to be atomic
//     */
//    uint32_t offset = 0;
//#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
//    ASSERT(*(uint64_t *)(seg_dram_data + offset) == SEG_MAGIC);
//    offset += sizeof(uint64_t);
//#endif
//    while (offset < heap.seg_size) {
//        oit = (struct item *)(seg_dram_data + offset);
//        nit = (struct item *)(seg_pmem_data + offset);
//        item_relink(oit, nit);
//    }
//    _seg_wait_refcnt(seg_id_dram);
//    return true;
//}

/**
 * allocate a new segment from DRAM heap, advance nseg,
 * return the
 */
static inline int32_t
_seg_alloc(void)
{
    /* in the steady state, nseg should always be equal to max_nseg,
     * when the heap is not full, we believe concurrent allocating
     * is rare and we optimize for this assumption by doing optimistically
     * alloc first, if the seg_id is too large, roll back
     * */

    if (__atomic_load_n(&heap.nseg, __ATOMIC_RELAXED) >= heap.max_nseg) {
        return -1;
    }

    int32_t seg_id = __atomic_fetch_add(&heap.nseg, 1, __ATOMIC_RELAXED);

    if (seg_id >= heap.max_nseg) {
        /* this is very rare, roll back */
        __atomic_fetch_sub(&heap.nseg, 1, __ATOMIC_RELAXED);
        return -1;
    }

    INCR(seg_metrics, seg_curr_dram);

    return seg_id;
}


static inline int32_t
_seg_get_from_free_pool(void)
{
    if (heap.free_seg_id == -1) {
        /* it is fine that we get race condition here */
        return -1;
    }

    log_debug("get seg from global free pool");
    int seg_id_ret, next_seg_id;

    pthread_mutex_lock(&heap.mtx);

    seg_id_ret = heap.free_seg_id;
    next_seg_id = heap.segs[seg_id_ret].next_seg_id;
    heap.free_seg_id = next_seg_id;
    if (next_seg_id != -1){
        heap.segs[next_seg_id].prev_seg_id = -1; /* not necessary */
    }

    pthread_mutex_unlock(&heap.mtx);

    heap.segs[seg_id_ret].in_free_pool = 0;

    log_debug("get seg %" PRId32 " from global free pool", seg_id_ret);

    return seg_id_ret;
}

/* return evicted seg to global pool, this function is called
 * after caller grabbed the lock */
void
seg_return_seg(int32_t seg_id)
{
    log_debug("return seg %" PRId32 " to global free pool", seg_id);

    int32_t curr_seg_id, next_seg_id;
    struct seg *seg = &heap.segs[seg_id];
    seg->next_seg_id = -1;
    seg->in_free_pool = 1;

    /* jason: I feel like lock is the best solution,
     * it should not cause scalability issue */

    curr_seg_id = heap.free_seg_id;
    if (curr_seg_id == -1) {
        heap.free_seg_id = seg_id;
        seg->prev_seg_id = -1;
    } else {
        next_seg_id = heap.segs[curr_seg_id].next_seg_id;
        while (next_seg_id != -1) {
            curr_seg_id = next_seg_id;
            next_seg_id = heap.segs[curr_seg_id].next_seg_id;
        }
        heap.segs[curr_seg_id].next_seg_id = seg_id;
        seg->prev_seg_id = curr_seg_id; /* not necessary */
    }


    log_vverb("return seg %" PRId32 " to free pool successfully", seg_id);
}

/*
 * alloc a seg from the seg pool, if there is no free segment, evict one
 *
 * this has become too complex, so only focus on DRAM for now
 */
int32_t
seg_get_new(void)
{
    /* TODO(jason): sync seg_header if we want to tolerate failures */
    evict_rstatus_e status;
    uint32_t seg_id_ret;

    INCR(seg_metrics, seg_req);

    if ((seg_id_ret = _seg_alloc()) != -1) {
        /* seg is allocated from heap */
        ;
    } else if ((seg_id_ret = _seg_get_from_free_pool()) != -1) {
        /* free pool does not have seg */
        ;
    } else {
        /* evict one seg */
        status = least_valuable_seg(&seg_id_ret);
        if (status == EVICT_NO_SEALED_SEG) {
            log_warn("unable to evict segment because no seg is sealed");
            INCR(seg_metrics, seg_req_ex);

            return -1;
        }
        seg_rm_all_item(seg_id_ret);
    }

    log_debug("seg_get_new: get segment %" PRIu32, seg_id_ret);

    _seg_init(seg_id_ret);
    return seg_id_ret;
}

static void
_heap_init(void)
{
    heap.nseg = 0;
    heap.max_nseg = heap.heap_size / heap.seg_size;
    heap.heap_size = heap.max_nseg * heap.seg_size;
    heap.base = NULL;

    if (!heap.prealloc) {
        log_crit("%s only support prealloc", SEG_MODULE_NAME);
        exit(EX_CONFIG);
    }
}

static int
_setup_heap_mem(void)
{
    int datapool_fresh = 1;

    heap.pool = datapool_open(heap.poolpath, heap.poolname, heap.heap_size,
            &datapool_fresh, false);
    if (heap.pool == NULL || datapool_addr(heap.pool) == NULL) {
        log_crit("create datapool failed: %s - %zu bytes for %" PRIu32 " segs",
                strerror(errno), heap.heap_size, heap.max_nseg);
        exit(EX_CONFIG);
    }
    log_info("pre-allocated %zu bytes for %" PRIu32 " segs", heap.heap_size,
            heap.max_nseg);

    heap.base = datapool_addr(heap.pool);

    return datapool_fresh;
}


/*
 * Initialize seg heap related info
 * we support the use of DRAM only or
 * PMem only (with hashtable and seg headers in DRAM or
 * both DRAM and PMem as a tiered storage, segments are migrated into PMem
 * using FIFO with hot segments bumped into DRAM
 *
 * NOTE: because we do not store header as part of the segment data
 * so when we calculate the max_nseg, we need to include the size of
 * headers,
 *
 *
 *
 *
 */
static rstatus_i
_seg_heap_setup(void)
{
    _heap_init();

    __attribute__((unused)) int dram_fresh = 1, pmem_fresh = 1;
    size_t seg_hdr_sz = SEG_HDR_SIZE * heap.max_nseg;

    dram_fresh = _setup_heap_mem();
    pthread_mutex_init(&heap.mtx, NULL);


    heap.segs = cc_zalloc(seg_hdr_sz);
    //    cc_memcpy(heap.segs, heap.persisted_seg_hdr, seg_hdr_sz);
    //    heap.reserved_seg = cc_zalloc(heap.seg_size);

    //    /* recover PMem first, because early recovered seg will migrate to
    //    PMem */ if (pmem_fresh == 0) {
    //        if (_seg_recovery(heap.base_pmem) != CC_OK) {
    //            /* TODO (jason): do we have to clear all seg and
    //            hashtable?
    //             * it depends on what causes the recovery failure though
    //             */
    //            log_warn("fail to recover items from pmem");
    //            goto fresh_start;
    //        }
    //    }
    //    if (dram_fresh == 0) {
    //        if (_seg_recovery(heap.base) != CC_OK) {
    //            log_warn("fail to recover items from DRAM");
    //            goto fresh_start;
    //        }
    //    }
    //    return CC_OK;

    // fresh_start:
    //    /* TODO(jason) clear hashtable, seg headers */
    //    for (uint32_t i = 0; i < heap.max_nseg + heap.max_nseg_pmem;
    //    i++) {
    //        _seg_init(i);
    //    }

    return CC_OK;
}


void
seg_rm_expired_seg(int32_t seg_id)
{
    log_debug("remove expired segment %" PRIu32, seg_id);
    seg_rm_all_item(seg_id);

    pthread_mutex_lock(&heap.mtx);
    seg_return_seg(seg_id);
    pthread_mutex_unlock(&heap.mtx);

//    _seg_init(seg_id);
}


void
seg_teardown(void)
{
    log_info("tear down the %s module", SEG_MODULE_NAME);

    if (!seg_initialized) {
        log_warn("%s has never been set up", SEG_MODULE_NAME);
        return;
    }

    hashtable_destroy(&hash_table);
    _sync_seg_hdr();

    segevict_teardown();
    locktable_teardown(&cas_table);
    ttl_bucket_teardown();

    seg_metrics = NULL;

    flush_at = -1;
    seg_initialized = false;
}

void
seg_setup(seg_options_st *options, seg_metrics_st *metrics)
{
    log_info("set up the %s module", SEG_MODULE_NAME);

    if (seg_initialized) {
        log_warn("%s has already been set up, re-creating", SEG_MODULE_NAME);
        seg_teardown();
    }

    log_info("Seg header size: %d, item header size: %d", SEG_HDR_SIZE,
            ITEM_HDR_SIZE);

    seg_metrics = metrics;

    if (options == NULL) {
        log_crit("no option is provided for seg initialization");
        exit(EX_CONFIG);
    }

    flush_at = -1;

    seg_options = options;
    heap.seg_size = option_uint(&options->seg_size);
    heap.heap_size = option_uint(&options->seg_mem);
    log_verb("DRAM size %" PRIu64, heap.heap_size);

    heap.free_seg_id = -1;
    heap.prealloc = option_bool(&seg_options->seg_prealloc);
    heap.prefault = option_bool(&seg_options->prefault);

    heap.poolpath = option_str(&seg_options->datapool_path);
    heap.poolname = option_str(&seg_options->datapool_name);

    use_cas = option_bool(&options->seg_use_cas);

    hash_power = option_uint(&seg_options->seg_hash_power);
    hash_table = hashtable_create(hash_power);
    if (hash_table == NULL) {
        log_crit("Could not create hash table");
        goto error;
    }

    if (_seg_heap_setup() != CC_OK) {
        log_crit("Could not setup seg heap info");
        goto error;
    }

    ttl_bucket_setup();

    locktable_create(&cas_table, LOCKTABLE_HASHPOWER);

    segevict_setup(option_uint(&options->seg_evict_opt), heap.max_nseg);


    seg_initialized = true;

    return;

error:
    seg_teardown();
    exit(EX_CONFIG);
}
