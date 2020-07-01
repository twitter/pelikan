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


static bool seg_initialized = false;
seg_metrics_st *seg_metrics = NULL;
seg_options_st *seg_options = NULL;
seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];

proc_time_i flush_at = -1;


void
_seg_print(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    loga("seg id %" PRIu32 " seg size %zu, create_at time %" PRId32
         ", ttl %" PRId32 ", initialized %u, sealed %u, in_pmem %u, "
         "%" PRIu32 " items, write offset %" PRIu32 ", occupied size "
         "%" PRIu32 ", n_hit %" PRIu32 ", n_hit_last %" PRIu32,
            seg->seg_id, heap.seg_size, seg->create_at, seg->ttl,
            seg->initialized, seg->sealed, seg->in_pmem, seg->n_item,
            seg->write_offset, seg->occupied_size, seg->n_hit, seg->n_hit_last);
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

/* we do not need to unlock a seg, because seg is locked for eviction/removal,
 * we do not need to explicitly unlock it */
static inline void
_seg_unlock(uint32_t seg_id)
{
    ;
}

/*
 * wait until the seg is available to be freed (refcount == 0)
 */
static inline void
_seg_wait_refcnt(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    ASSERT(seg->locked != 0);

    log_vverb("wait for seg %" PRIu32 " lock", seg_id);
    /* TODO (jason): a better way to spin lock here? */
    while (__atomic_load_n(&(seg->refcount), __ATOMIC_RELAXED) > 0) {
        ;
    }
    log_vverb("wait for seg %" PRIu32 " lock finishes", seg_id);
}

static inline bool
_seg_heap_full()
{
    bool dram_full = heap.nseg_dram >= heap.max_nseg_dram;
    bool pmem_full = heap.nseg_pmem >= heap.max_nseg_pmem;
    return dram_full & pmem_full;
}

static inline bool
_seg_dram_heap_full()
{
    return heap.nseg_dram >= heap.max_nseg_dram;
}

static inline bool
_seg_pmem_heap_full()
{
    return heap.nseg_pmem >= heap.max_nseg_pmem;
}

static void
_sync_seg_hdr(void)
{
    log_crit("wait for impl :(");
    //    cc_memcpy(heap.persisted_seg_hdr, heap.segs,
    //            SEG_HDR_SIZE * (heap.max_nseg_dram + heap.max_nseg_pmem));
}

///*
// * Because a seg can have a reserved item (claimed but not linked), which is
// * requested when a write command does not have the entirety of value in the
// * buffer, eviction will fail if the seg has a non-zero refcount. True is
// * returned if the seg got no reserved items, otherwise False.
// */
// static inline bool
//_seg_check_no_refcount(struct seg *seg)
//{
//    return (seg->refcount == 0);
//}

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
_seg_recover_seg_header()
{
    uint32_t i;

    for (i = 0; i < heap.max_nseg_dram; i++) {
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
    if (old_seg_id >= heap.max_nseg_dram) {
        old_seg_data = heap.base_pmem +
                heap.seg_size * (old_seg_id - heap.max_nseg_dram);
    } else {
        old_seg_data = heap.base_dram + heap.seg_size * old_seg_id;
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
_seg_init(uint32_t seg_id)
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
    seg->initialized = 1;
    seg->in_pmem = seg_id >= heap.max_nseg_dram ? 1 : 0;
    seg->create_at = 0;
    seg->locked = 0;
}

/*
 * remove all items on this segment, at this time, the seg should be sealed
 * indicating the seg hsa no writer, but can have readers, doing so allows us
 * to avoid locking to access the metadata of the seg
 *
 * we do not remove/evict un-sealed seg,
 * because doing so will cause new writes to be corrupted
 * as they are inserted into hashtable, but item data is wiped
 *
 * because multiple threads could try to wipe the seg at the same time,
 * return true if current thread is able to wipe this seg,
 * otherwise false
 */
bool
seg_rm_all_item(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    struct item *it;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    /* lock the seg to prevent other threads evicting this seg */
    if (!_seg_lock(seg_id)) {
        /* fail to lock, some other thread is expiring/evicting this seg */
        return false;
    }

    ASSERT(seg->write_offset <= heap.seg_size);
    while (curr - seg_data < seg->write_offset) {
        it = (struct item *)curr;
        curr += item_ntotal(it);

        struct bstring key = {.data = item_key(it), .len = item_nkey(it)};
        item_delete(&key);
    }
    /* all operation up till here does not require refcount to be 0
     * because the data on the segment is not cleared yet,
     * now we clear the segment data, we need to check refcount
     * since we have already locked the item before removing entries
     * from hashtable, ideally by the time we have removed all hashtable
     * entries, previous requests on this segment have all finished */
    _seg_wait_refcnt(seg_id);
    return true;
}

/* the segment points to by seg_id_pmem is empty and ready to use */
bool
migrate_dram_to_pmem(uint32_t seg_id_dram, uint32_t seg_id_pmem)
{
    /* first thing, we lock the dram seg to prevent future access to it */
    /* TODO(jason): change function signature to use struct seg instead of
     * seg_id */

    log_verb("migrate DRAM seg %" PRIu32 " to PMem seg %" PRIu32, seg_id_dram,
            seg_id_pmem);

    if (!_seg_lock(seg_id_dram)) {
        return false;
    }

    struct item *oit, *nit;
    struct seg *seg_dram = &heap.segs[seg_id_dram];
    struct seg *seg_pmem = &heap.segs[seg_id_pmem];
    uint8_t *seg_dram_data = seg_get_data_start(seg_id_dram);
    uint8_t *seg_pmem_data = seg_get_data_start(seg_id_pmem);

    cc_memcpy(seg_dram, seg_pmem, sizeof(struct seg));
    cc_memcpy(seg_pmem_data, seg_dram_data, heap.seg_size);

    seg_pmem->refcount = 0;
    seg_pmem->locked = 0;
    seg_pmem->seg_id = seg_id_pmem;
    seg_pmem->in_pmem = 1;

    /* relink hash table, this needs to be thread-safe
     * we don't need lock here,
     * since we require hashtable update to be atomic
     */
    uint32_t offset = 0;
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(seg_dram_data + offset) == SEG_MAGIC);
    offset += sizeof(uint64_t);
#endif
    while (offset < heap.seg_size) {
        oit = (struct item *)(seg_dram_data + offset);
        nit = (struct item *)(seg_pmem_data + offset);
        item_relink(oit, nit);
    }
    _seg_wait_refcnt(seg_id_dram);
}

/**
 * allocate a new segment from DRAM heap, advance nseg_dram,
 * return the
 */
static inline struct seg *
_seg_alloc_from_dram()
{
    ASSERT(!_seg_dram_heap_full());

    uint32_t seg_id = heap.nseg_dram++;

    INCR(seg_metrics, seg_curr_dram);

    return &heap.segs[seg_id];
}

static inline struct seg *
_seg_alloc_from_pmem()
{
    ASSERT(!_seg_pmem_heap_full());

    uint32_t seg_id = heap.nseg_pmem++;

    INCR(seg_metrics, seg_curr_pmem);

    return &heap.segs[seg_id];
}

/*
 * alloc a seg from the seg pool, if there is no free segment, evict one
 *
 * this has become too complex, so only focus on DRAM for now
 */
struct seg *
seg_get_new(void)
{
    /* TODO(jason): sync seg_header if we want to tolerate failures */
    struct seg *seg_ret;
    evict_rstatus_e status;
    uint32_t seg_id_dram, seg_id_pmem, seg_id_ret;

    INCR(seg_metrics, seg_req);

    if (!_seg_dram_heap_full()) {
        seg_id_ret = _seg_alloc_from_dram()->seg_id;
    } else if (!_seg_pmem_heap_full()) {
        /* TODO(jason): this is dangerous because we might have non-predictable
         * latency and limits scalability due to modification to hashtable */
        seg_id_ret = _seg_alloc_from_pmem()->seg_id;
        if (seg_use_dram()) {
            /* if both DRAM and PMem are used, migrate one DRAM segment to PMem
             */
            status = least_valuable_seg_dram(&seg_id_dram);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict DRAM segment because no seg is "
                         "sealed");
                INCR(seg_metrics, seg_req_ex);
            } else {
                migrate_dram_to_pmem(seg_id_dram, seg_id_ret);
                seg_id_ret = seg_id_dram;
            }
        }
    } else {
        /* both DRAM and PMem are full (or not used),
         * we have to evict in this case */
        if (seg_use_pmem()) {
            /* if PMem is used, we evict from PMem */
            status = least_valuable_seg_pmem(&seg_id_pmem);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict PMem segment because no seg is "
                         "sealed");
                INCR(seg_metrics, seg_req_ex);

                return NULL;
            }

            /* TODO(jason): BUG!! we need to check the return val
             * it can be false when mulitple threads are trying to evict
             * the same seg */
            seg_rm_all_item(seg_id_pmem);
            status = least_valuable_seg_dram(&seg_id_dram);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict segment because no seg is sealed");
                INCR(seg_metrics, seg_req_ex);

                return NULL;
            }
            migrate_dram_to_pmem(seg_id_dram, seg_id_pmem);
        } else {
            /* PMem is not used, we can only evict from DRAM */
            for (uint x = 0; x < 4; x++)
                _seg_print(x);
            status = least_valuable_seg_dram(&seg_id_dram);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict segment because no seg is sealed");
                INCR(seg_metrics, seg_req_ex);

                return NULL;
            }

            log_verb("evict DRAM segment %" PRIu32, seg_id_dram);
            seg_rm_all_item(seg_id_dram);
        }
        seg_id_ret = seg_id_dram;
    }
    log_verb("get segment %" PRIu32, seg_id_ret);

    _seg_init(seg_id_ret);
    /* TODO(jason): we may want to change all seg functions to return only
     * seg_id or seg* */
    seg_ret = &heap.segs[seg_id_ret];
    seg_ret->create_at = time_proc_sec();
    return seg_ret;
}

static void
_heap_init()
{
    heap.nseg_dram = 0;
    heap.max_nseg_dram = heap.size_dram / heap.seg_size;
    heap.size_dram = heap.max_nseg_dram * heap.seg_size;
    heap.base_dram = NULL;

    if (heap.size_pmem > 0) {
        heap.nseg_pmem = 0;
        /* when PMem is used,
         * store the persisted DRAM + PMem seg headers on PMem */
        heap.max_nseg_pmem = heap.size_pmem / heap.seg_size;
        heap.size_pmem = heap.max_nseg_pmem * heap.seg_size;
        heap.base_pmem = NULL;
    }

    if (!heap.prealloc) {
        log_crit("%s only support prealloc", SEG_MODULE_NAME);
        exit(EX_CONFIG);
    }
}

static int
_setup_dram_heap()
{
    int datapool_fresh = 1;

    heap.pool_dram = datapool_open(heap.poolpath_dram, heap.poolname_dram,
            heap.size_dram, &datapool_fresh, false);
    if (heap.pool_dram == NULL || datapool_addr(heap.pool_dram) == NULL) {
        log_crit("create DRAM datapool failed: %s - %zu bytes for %" PRIu32 " s"
                 "eg"
                 "s",
                strerror(errno), heap.size_dram, heap.max_nseg_dram);
        exit(EX_CONFIG);
    }
    log_info("pre-allocated %zu bytes for %" PRIu32 " segs", heap.size_dram,
            heap.max_nseg_dram);

    if (seg_use_pmem() == 0) {
        /* only DRAM is used, the persisted seg header are in DRAM, so
         * we reserve the first mem_seg_hdr_sz bytesfor seg headers */
        heap.base_dram = datapool_addr(heap.pool_dram);
    } else {
        heap.base_dram = datapool_addr(heap.pool_dram);
    }
    return datapool_fresh;
}

static int
_setup_pmem_heap()
{
    int datapool_fresh = 1;

    heap.pool_pmem = datapool_open(heap.poolpath_pmem, heap.poolname_pmem,
            heap.size_pmem, &datapool_fresh, heap.prefault);

    if (heap.pool_pmem == NULL || datapool_addr(heap.pool_pmem) == NULL) {
        log_crit("create PMem datapool failed: %s - allocating %zu bytes for "
                 "%" PRIu32 " segs",
                strerror(errno), heap.size_pmem, heap.max_nseg_pmem);
        exit(EX_CONFIG);
    }
    log_info("pre-allocated %zu bytes for %" PRIu32 " segs", heap.size_pmem,
            heap.max_nseg_pmem);

    /* the first mem_seg_hdr_sz bytes are reserved for seg headers */
    heap.base_pmem = datapool_addr(heap.pool_pmem);

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
 * so when we calculate the max_nseg, we need to include the size of headers,
 *
 *
 *
 *
 */
static rstatus_i
_seg_heap_setup()
{
    _heap_init();

    int dram_fresh = 1, pmem_fresh = 1;
    uint32_t n_segs = heap.max_nseg_dram + heap.max_nseg_pmem;
    size_t seg_hdr_sz = SEG_HDR_SIZE * n_segs;

    if (heap.max_nseg_dram > 0) {
        dram_fresh = _setup_dram_heap();
    }

    if (heap.max_nseg_pmem > 0) {
        pmem_fresh = _setup_pmem_heap();
    }

    heap.segs = cc_zalloc(seg_hdr_sz);
    //    cc_memcpy(heap.segs, heap.persisted_seg_hdr, seg_hdr_sz);
    //    heap.reserved_seg = cc_zalloc(heap.seg_size);

    //    /* recover PMem first, because early recovered seg will migrate to
    //    PMem */ if (pmem_fresh == 0) {
    //        if (_seg_recovery(heap.base_pmem) != CC_OK) {
    //            /* TODO (jason): do we have to clear all seg and hashtable?
    //             * it depends on what causes the recovery failure though
    //             */
    //            log_warn("fail to recover items from pmem");
    //            goto fresh_start;
    //        }
    //    }
    //    if (dram_fresh == 0) {
    //        if (_seg_recovery(heap.base_dram) != CC_OK) {
    //            log_warn("fail to recover items from DRAM");
    //            goto fresh_start;
    //        }
    //    }
    //    return CC_OK;

fresh_start:
    /* TODO(jason) clear hashtable, seg headers */
    for (uint32_t i = 0; i < heap.max_nseg_dram + heap.max_nseg_pmem; i++) {
        _seg_init(i);
    }

    return CC_OK;
}


void
seg_rm_expired_seg(uint32_t seg_id)
{
    seg_rm_all_item(seg_id);
    _seg_init(seg_id);
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
    heap.size_dram = option_uint(&options->seg_mem_dram);
    heap.size_pmem = option_uint(&options->seg_mem_pmem);
    ASSERT(heap.size_dram + heap.size_pmem > 0);
    log_verb("DRAM size %" PRIu64 ", PMem size %" PRIu64, heap.size_dram,
            heap.size_pmem);

    heap.prealloc = option_bool(&seg_options->seg_prealloc);
    heap.prefault = option_bool(&seg_options->prefault_pmem);

    heap.poolpath_dram = option_str(&seg_options->datapool_path_dram);
    heap.poolname_dram = option_str(&seg_options->datapool_name_dram);
    heap.poolpath_pmem = option_str(&seg_options->datapool_path_pmem);
    heap.poolname_pmem = option_str(&seg_options->datapool_name_pmem);

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

    segevict_setup(option_uint(&options->seg_evict_opt), heap.max_nseg_dram,
            heap.max_nseg_pmem);


    seg_initialized = true;

    return;

error:
    seg_teardown();
    exit(EX_CONFIG);
}
