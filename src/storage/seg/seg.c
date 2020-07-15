#include "seg.h"
#include "background.h"
#include "constant.h"
#include "datapool/datapool.h"
#include "hashtable2.h"
#include "item.h"
#include "segevict.h"
#include "ttlbucket.h"

#include <cc_mm.h>
#include <cc_util.h>

#include <errno.h>
#include <inttypes.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

#define SEG_MODULE_NAME "storage::seg"
#define TRIES_MAX 10

extern struct setting setting;
extern struct seg_evict_info evict;
pthread_t bg_tid;

struct seg_heapinfo heap; /* info of all allocated segs */
struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];
// struct hash_table *hash_table = NULL;

static bool seg_initialized = false;
seg_metrics_st *seg_metrics = NULL;
seg_options_st *seg_options = NULL;
seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];

proc_time_i flush_at = -1;
bool use_cas = false;
volatile bool stop = false;

// pthread_mutex_t seg_free_pool_mtx;


void
seg_print(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    log_debug("seg %" PRId32 " seg size %zu, create_at time %" PRId32
              ", ttl %" PRId32 ", locked %u, %" PRIu32 " items, write offset "
              "%" PRIu32 ", occupied size %" PRIu32 ", n_hit %" PRIu32
              ", n_hit_last %" PRIu32 ", read refcount %u, write refcount %u, "
              "prev_seg %" PRId32 ", next_seg %" PRId32,
            seg->seg_id, heap.seg_size, seg->create_at, seg->ttl,
            __atomic_load_n(&(seg->locked), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->n_item), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->write_offset), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->occupied_size), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->n_hit), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->n_hit_last), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->r_refcount), __ATOMIC_SEQ_CST),
            __atomic_load_n(&(seg->w_refcount), __ATOMIC_SEQ_CST),
            seg->prev_seg_id, seg->next_seg_id);
}

static inline void
_debug_print_seg_list(void)
{
    for (int i = 0; i < MAX_TTL_BUCKET; i++) {
        struct ttl_bucket *ttl_bucket = &ttl_buckets[i];
        if (ttl_bucket->first_seg_id == -1) {
            continue;
        }
        log_debug("ttl bucket %d first seg %d last seg %d\n", i,
                ttl_bucket->first_seg_id, ttl_bucket->last_seg_id);
    }
    for (int i = 0; i < heap.max_nseg; i++)
        log_debug("seg %d: prev %d next %d\n", i, heap.segs[i].prev_seg_id,
                heap.segs[i].next_seg_id);
}

void
dump_seg_info(void)
{
    uint32_t seg_id;
    for (int i = 0; i < MAX_TTL_BUCKET; i++) {
        struct ttl_bucket *ttl_bucket = &ttl_buckets[i];
        if (ttl_bucket->first_seg_id == -1) {
            continue;
        }
        printf("ttl bucket %d (%" PRId32 ") first seg %d last seg %d, "
               "seg_id/create_at/n_hit",
                i, ttl_bucket->ttl, ttl_bucket->first_seg_id,
                ttl_bucket->last_seg_id);
        seg_id = ttl_bucket->first_seg_id;
        while (seg_id != -1) {
            printf("%" PRIu32 "/%" PRId32 "/%" PRId32 ", ",
                    heap.segs[seg_id].seg_id, heap.segs[seg_id].create_at,
                    heap.segs[seg_id].n_hit);
            seg_id = (uint64_t)heap.segs[seg_id].next_seg_id;
        }
        printf("\n");
    }
}

/* when we want to evict/remove a seg, we need to make sure no other
 * threads are still reading from the seg, so
 * 1. we locked the seg to prevent future access,
 * 2. perform non-destructive eviction op (such as remove hashtable entries)
 * 3. then we check and wait until refcount becomes 0
 * 4. init the seg for future use */
// static inline bool
//_seg_lock(uint32_t seg_id)
//{
//    struct seg *seg = &heap.segs[seg_id];
//    return __atomic_exchange_n(&seg->locked, 1, __ATOMIC_SEQ_CST) == 0 ? true
//    :
//           false;
//}

/*
 * wait until the seg is available to be freed (refcount == 0)
 */
static inline void
_seg_wait_refcnt(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    ASSERT(seg->locked != 0);
    bool r_log_printed = false, w_log_printed = false;
    int r_ref, w_ref;

    w_ref = __atomic_load_n(&(seg->w_refcount), __ATOMIC_SEQ_CST);
    r_ref = __atomic_load_n(&(seg->r_refcount), __ATOMIC_SEQ_CST);

    if (w_ref) {
        log_verb("wait for seg %d refcount, current read refcount "
                 "%d, write refcount %d",
                seg_id, r_ref, w_ref);
        w_log_printed = true;
    }

    while (w_ref) {
        sched_yield();
        w_ref = __atomic_load_n(&(seg->w_refcount), __ATOMIC_SEQ_CST);
    }


    if (r_ref) {
        log_verb("wait for seg %d refcount, current read refcount "
                 "%d, write refcount %d",
                seg_id, r_ref, w_ref);
        r_log_printed = true;
    }

    while (r_ref) {
        sched_yield();
        r_ref = __atomic_load_n(&(seg->r_refcount), __ATOMIC_SEQ_CST);
    }


    if (r_log_printed || w_log_printed)
        log_verb("wait for seg %d refcount finishes", seg_id);
}


bool
seg_expired(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    /* seg->locked == 1 means being evicted, should not read it */
    uint8_t locked = __atomic_load_n(&seg->locked, __ATOMIC_SEQ_CST);
    bool expired = (locked) || seg->ttl + seg->create_at < time_proc_sec();
    expired = expired || seg->create_at <= flush_at;

    if (expired && !locked) {
        seg_rm_expired_seg(seg->seg_id);
    }
    return expired;
}

bool
seg_r_ref(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    if (__atomic_load_n(&seg->locked, __ATOMIC_SEQ_CST) == 0) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->r_refcount, 1, __ATOMIC_SEQ_CST);
        return true;
    }

    return false;
}

void
seg_r_deref(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->r_refcount, 1, __ATOMIC_SEQ_CST);

    ASSERT(ref >= 0);
}

bool
seg_w_ref(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    if (__atomic_load_n(&seg->locked, __ATOMIC_SEQ_CST) == 0) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->w_refcount, 1, __ATOMIC_SEQ_CST);
        return true;
    }

    return false;
}

void
seg_w_deref(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->w_refcount, 1, __ATOMIC_SEQ_CST);

    ASSERT(ref >= 0);
}


static inline void
_sync_seg_hdr(void)
{
    log_crit("wait for impl :(");
}


static void
_seg_init(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    uint8_t *data_start = seg_get_data_start(seg_id);

    cc_memset(data_start, 0, heap.seg_size);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    *(uint64_t *)(data_start) = SEG_MAGIC;
    seg->write_offset = 8;
    seg->occupied_size = 8;
#endif

    /* does not need lock, because this seg either
     * comes from un-allocated heap, freepool or eviction
     * and owned by only current thread,
     * the one except eviction algorithm may read the header */
    seg->prev_seg_id = -1;
    seg->next_seg_id = -1;

    seg->n_item = 0;
    seg->n_hit = 0;
    seg->n_hit_last = 0;

    seg->seg_id = seg_id;
    //    __atomic_store_n(&seg->next_seg_id, -1, __ATOMIC_RELEASE);
    seg->create_at = time_proc_sec();

    //    /* this needs to be after setting next_seg_id = -1
    //     * otherwise, eviction can pick this seg before it is
    //     * linked into ttl bucket */
    seg->in_free_pool = 0;
    //    __atomic_store_n(&seg->in_free_pool, 0, __ATOMIC_SEQ_CST);

    /* we set seg unlocked only after it is linked to ttl_bucket seg list
     * in other words: locked indicate the seg cannot be evicted */
    ASSERT(seg->locked == 1);
}

static uint32_t
_scan_active_items(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    struct item *it;
    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    uint32_t offset = __atomic_load_n(&seg->write_offset, __ATOMIC_SEQ_CST);

    ASSERT(seg->locked == 1);
    ASSERT(__atomic_load_n(&seg->locked, __ATOMIC_SEQ_CST) == 1);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    //    sleep(1);

    uint64_t n_item = 0;
    uint64_t seg_n_item = seg->n_item;
    uint64_t n_item_updated = 0;
    struct item *found_it;
    uint32_t seg_id_get = 0;

    while (curr - seg_data < offset) {
        it = (struct item *)curr;
        curr += item_ntotal(it);
        found_it =
                hashtable_get(item_key(it), item_nkey(it), &seg_id_get, NULL);
        if (it == found_it) {
            n_item += 1;
            ASSERT(seg_id == seg_id_get);
        } else if (found_it != NULL) {
            n_item_updated += 1;
        }
    }
    log_debug("%d %d %d %d", seg_id, seg_n_item, n_item, n_item_updated);
    ASSERT(offset == __atomic_load_n(&seg->write_offset, __ATOMIC_SEQ_CST));
    ASSERT(seg_n_item == seg->n_item);
    seg_n_item = seg->n_item;
    ASSERT(n_item == seg_n_item);

    return 0;
}

void
_rm_seg_from_ttl_bucket(uint32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    struct ttl_bucket *ttl_bucket = &ttl_buckets[find_ttl_bucket_idx(seg->ttl)];

    /* all modification to seg list needs to be protected by lock */
    pthread_mutex_lock(&heap.mtx);

    int32_t prev_seg_id = seg->prev_seg_id;
    int32_t next_seg_id = seg->next_seg_id;

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

    heap.segs[seg_id].next_seg_id = -1;

    ttl_bucket->n_seg -= 1;
    ASSERT(ttl_bucket->n_seg >= 0);

    pthread_mutex_unlock(&heap.mtx);
}

/*
 * remove all items on this segment,
 * most of the time (common case), the seg should have no writers because
 * the eviction algorithms will avoid the segment with w_refcnt > 0 or
 * next_seg_id == -1 (active segment)
 *
 * However, it is possible we are evicting a segment being written to in the
 * following cases:
 * 1. it takes too long (longer than its TTL) for the segment to
 *      finish writing and it has expired
 * 2. cache size is too small and the workload uses too many ttl buckets
 *
 *
 * because multiple threads could try to evict/expire the seg at the same time,
 * return true if current thread is able to grab the lock, otherwise false
 */
bool
seg_rm_all_item(uint32_t seg_id, int expire)
{
    static char *eviction_reasons[] = {"evict", "expire"};
    struct seg *seg = &heap.segs[seg_id];
    struct item *it;

    /* lock the seg to prevent other threads accessing and evicting this
     * segment, this lock is not released until
     * 1. all hash table entries are removed, so no future access
     * 2. the next_seg_id becomes -1 (end of evict and init) so that it
     *      will not be picked for eviction soon
     *
     * we do all the computation after lock the seg, this gives two benefits
     * 1. lock as early as possible to refuse all future accesses and
     * 2. wait for readers while doing useful work
     * */

    //    if (__atomic_test_and_set(&seg->locked, __ATOMIC_SEQ_CST)) {
    if (__atomic_exchange_n(&seg->locked, 1, __ATOMIC_SEQ_CST) == 1) {
        /* fail to lock, either because it is in the free pool or
         * some other thread is expiring/evicting this seg */
        log_warn("%s seg %" PRIu32 ": unable to lock seg, ttl %" PRId32,
                eviction_reasons[expire], seg_id, expire, seg->ttl);
        INCR(seg_metrics, seg_evict_ex);

        /* if most ttl are very small, during benchmark !expire may not hold */
        ASSERT((!expire) || seg->ttl < 60);

        return false;
    }

    /* next_seg_id == -1 indicates this is the last segment of a ttl_bucket
     * or freepool, and we should not evict the seg
     * we have tried to avoid picking such seg at eviction, but it can still
     * happen because
     * 1. this seg has been evicted and reused by another thread since it was
     *      picked by eviction algorithm (because there is no lock) - very rare
     * 2. this seg is expiring, so we have to evict it
     * either case should be rare, it is the effect of
     * optimistic concurrency control - no lock and roll back if needed
     *
     * since we have already "locked" the seg, it will not be held by other
     * threads, so we can check again safely
     */
    if (seg->next_seg_id == -1 && (!expire)) {
        //        __atomic_clear(&seg->locked, __ATOMIC_SEQ_CST);
        __atomic_store_n(&seg->locked, 0, __ATOMIC_SEQ_CST);

        log_warn("%s seg %" PRIu32 ": next_seg has been changed, give up",
                eviction_reasons[expire], seg_id);
        INCR(seg_metrics, seg_evict_ex);

        return false;
    }

    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    uint32_t offset = __atomic_load_n(&seg->write_offset, __ATOMIC_SEQ_CST);

    log_debug("proc time %" PRId32 ": %s seg %" PRId32 ", ttl %d",
            time_proc_sec(), eviction_reasons[expire], seg_id, seg->ttl);
    seg_print(seg_id);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    _rm_seg_from_ttl_bucket(seg_id);

    //    _scan_active_items(seg_id);

    int n_del = 0;
    bool found;
    while (curr - seg_data < offset) {
        it = (struct item *)curr;
        //        item_delete_it(it);
        found = hashtable_evict(
                item_key(it), it->klen, seg_id, curr - seg_data);
        if (found)
            n_del += 1;
        curr += item_ntotal(it);
    }

    ASSERT(__atomic_load_n(&seg->n_item, __ATOMIC_SEQ_CST) >= 0);

    /* all operation up till here does not require refcount to be 0
     * because the data on the segment is not cleared yet,
     * now we are ready to clear the segment data, we need to check refcount
     * because we have already locked the segment before removing entries
     * from hashtable, ideally by the time we have removed all hashtable
     * entries, all previous requests on this segment have all finished */
    _seg_wait_refcnt(seg_id);

    /* optimistic concurrency control:
     * because we didn't wait for refcount before remove hashtable entries
     * it is possible that there are some very slow writers, which finish
     * writing (_item_define) after we clear the hashtable entries,
     * so we need to double check, in most cases, this should not happen */

    if (__atomic_load_n(&seg->n_item, __ATOMIC_SEQ_CST) > 0) {
        INCR(seg_metrics, seg_evict_retry);
        /* because we don't know which item is newly written, so we
         * have to remove all items again */
        curr = seg_data;
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        curr += sizeof(uint64_t);
#endif
        while (curr - seg_data < offset) {
            it = (struct item *)curr;
            hashtable_evict(item_key(it), it->klen, seg_id, curr - seg_data);
            curr += item_ntotal(it);
            //            item_delete_it(it);
        }
    }

    //    if (seg->n_item != 0) {
    //        sleep(2);
    //        bool in_cache;
    //        curr = seg_data;
    //#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    //        curr += sizeof(uint64_t);
    //#endif
    //        while (curr - seg_data < offset) {
    //            it = (struct item *)curr;
    //            hashtable_evict(item_key(it), it->klen, seg_id, curr -
    //            seg_data); curr += item_ntotal(it);
    //            //            in_cache = item_delete_it(it);
    //
    //            //            if (in_cache)
    //            //                ASSERT(0);
    //        }
    //        printf("slept checked\n");
    //    }
    ASSERT(seg->n_item == 0);

    if (expire){
        INCR(seg_metrics, seg_expire);
    } else {
    INCR(seg_metrics, seg_evict);
    }

    return true;
}


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

    if (__atomic_load_n(&heap.nseg, __ATOMIC_SEQ_CST) >= heap.max_nseg) {
        return -1;
    }

    uint32_t seg_id = __atomic_fetch_add(&heap.nseg, 1, __ATOMIC_SEQ_CST);

    if (seg_id >= heap.max_nseg) {
        /* this is very rare, roll back */
        __atomic_fetch_sub(&heap.nseg, 1, __ATOMIC_SEQ_CST);
        return -1;
    }

    INCR(seg_metrics, seg_curr_dram);

    return seg_id;
}

static inline void
_print_free_seg_list(char *msg)
{
    if (heap.free_seg_id == -1) {
        log_debug("%s: free seg list: empty", msg);
        return;
    }

    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    char str[2048];
    struct seg *prev_seg = NULL;
    struct seg *curr_seg = &heap.segs[heap.free_seg_id];
    int cnt = 0, pos = 0;

    while (cnt < 200) {
        pos += snprintf(str + pos, 8, "%d=>", curr_seg->seg_id);
        cnt += 1;
        if (curr_seg->next_seg_id == -1) {
            break;
        }
        ASSERT(curr_seg->next_seg_id ==
                heap.segs[curr_seg->next_seg_id].seg_id);
        prev_seg = curr_seg;
        curr_seg = &heap.segs[curr_seg->next_seg_id];
        ASSERT(curr_seg->prev_seg_id == prev_seg->seg_id);
    }
    if (cnt == 200) {
        snprintf(str + pos, 4, "....");
    } else {
        snprintf(str + pos, 4, "end.");
    }
    log_debug("%s: free seg list: %s", msg, str);
}

static inline int32_t
_seg_get_from_free_pool(void)
{
    int seg_id_ret, next_seg_id;

    int status;

    status = pthread_mutex_lock(&heap.mtx);

    if (status != 0) {
        log_warn("fail to lock seg free pool");
        /* TODO(jason): clean up */
        return -1;
    }

    //    _print_free_seg_list("get_from_free_pool1");

    seg_id_ret = heap.free_seg_id;

    if (seg_id_ret == -1) {
        pthread_mutex_unlock(&heap.mtx);

        return -1;
    }

    ASSERT(seg_id_ret >= 0);

    next_seg_id = heap.segs[seg_id_ret].next_seg_id;
    heap.free_seg_id = next_seg_id;
    if (next_seg_id != -1) {
        heap.segs[next_seg_id].prev_seg_id = -1; /* not necessary */
    }

    ASSERT(heap.segs[seg_id_ret].write_offset == 0 ||
            heap.segs[seg_id_ret].write_offset == 8);
    ASSERT(heap.segs[seg_id_ret].in_free_pool == 1);

    //    _print_free_seg_list("get_from_free_pool2");

    pthread_mutex_unlock(&heap.mtx);

    return seg_id_ret;
}


/**
 * return evicted seg to global pool,
 * caller should grab the global lock before calling this function
 **/
void
seg_return_seg(uint32_t seg_id)
{
    log_debug("return seg %d to global free pool", seg_id);

    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    //    _print_free_seg_list("return_to_free_pool1");

    /* TODO(jason): change to add at head */

    int32_t curr_seg_id, next_seg_id;
    struct seg *seg = &heap.segs[seg_id];
    seg->locked = 1; /* to prevent being evicted */
    seg->next_seg_id = -1;
    seg->in_free_pool = 1;
    /* we set all free segs as locked to prevent it being evicted
     * before finishing setup */
    //    seg->locked = 1;
    __atomic_store_n(&seg->locked, 1, __ATOMIC_SEQ_CST);
    /* can we remove the following lines ? */
    __atomic_store_n(&seg->write_offset, 0, __ATOMIC_SEQ_CST);
    __atomic_store_n(&seg->occupied_size, 0, __ATOMIC_SEQ_CST);
    //    seg->write_offset = 0;
    //    seg->occupied_size = 0;

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


    //    _print_free_seg_list("return_to_free_pool2");


    log_vverb("return seg %" PRId32 " to free pool successfully", seg_id);
}

/**
 * alloc a seg from the seg pool, if there is no free segment, evict one
 **/
int32_t
seg_get_new(void)
{
    /* TODO(jason): sync seg_header if we want to tolerate failures */
    evict_rstatus_e status;
    uint32_t seg_id_ret;

    INCR(seg_metrics, seg_req);

    if ((seg_id_ret = _seg_alloc()) != -1) {
        /* seg is allocated from heap */
        log_debug("seg_get_new: allocate seg %" PRIu32 " from unused heap",
                seg_id_ret);
    } else if ((seg_id_ret = _seg_get_from_free_pool()) != -1) {
        /* free pool has seg */
        log_debug("seg_get_new: allocate seg %" PRIu32 " from free pool",
                seg_id_ret);
    } else {
        /* evict one seg */
        int n_evict_retries = 0;
        while (1) {
            /* eviction may fail if other threads pick the same seg
             * (can happen in random eviction */
            status = least_valuable_seg(&seg_id_ret);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict seg because no seg is sealed");
                INCR(seg_metrics, seg_req_ex);

                return -1;
            }
            log_debug("going to evict seg %" PRId32, seg_id_ret);
            if (seg_rm_all_item(seg_id_ret, 0)) {
                log_debug("seg_get_new: allocate seg %" PRIu32 " from "
                          "eviction",
                        seg_id_ret);
                break;
            }

            if (++n_evict_retries >= 3) {
                log_warn("seg_get_new: unable to evict after retries");
                return -1;
            }
        }
    }

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


static rstatus_i
_seg_heap_setup(void)
{
    _heap_init();

    int dram_fresh = 1;
    size_t seg_hdr_sz = SEG_HDR_SIZE * heap.max_nseg;

    dram_fresh = _setup_heap_mem();
    pthread_mutex_init(&heap.mtx, NULL);

    heap.segs = cc_zalloc(seg_hdr_sz);

    if (!dram_fresh) {
        /* TODO(jason): recover */
        ;
    } else {
        for (int32_t i = 0; i < heap.max_nseg; i++) {
            heap.segs[i].seg_id = i;
            heap.segs[i].locked = 1;
        }
    }

    return CC_OK;
}

void
seg_rm_expired_seg(uint32_t seg_id)
{
    bool success = seg_rm_all_item(seg_id, 1);
    if (!success) {
        return;
    }

    pthread_mutex_lock(&heap.mtx);
    seg_return_seg(seg_id);
    pthread_mutex_unlock(&heap.mtx);
}

void
seg_teardown(void)
{
    log_info("tear down the %s module", SEG_MODULE_NAME);

    stop = true;

    pthread_join(bg_tid, NULL);

    if (!seg_initialized) {
        log_warn("%s has never been set up", SEG_MODULE_NAME);
        return;
    }

    hashtable_teardown();
    _sync_seg_hdr();

    segevict_teardown();
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
    stop = false;

    seg_options = options;
    heap.seg_size = option_uint(&options->seg_size);
    heap.heap_size = option_uint(&options->seg_mem);
    log_verb("cache size %" PRIu64, heap.heap_size);

    heap.free_seg_id = -1;
    heap.prealloc = option_bool(&seg_options->seg_prealloc);
    heap.prefault = option_bool(&seg_options->prefault);

    heap.poolpath = option_str(&seg_options->datapool_path);
    heap.poolname = option_str(&seg_options->datapool_name);

    use_cas = option_bool(&options->seg_use_cas);

    hashtable_setup(option_uint(&seg_options->seg_hash_power));

    if (_seg_heap_setup() != CC_OK) {
        log_crit("Could not setup seg heap info");
        goto error;
    }

    ttl_bucket_setup();

    segevict_setup(option_uint(&options->seg_evict_opt), heap.max_nseg);

    //    pthread_mutex_init(&seg_free_pool_mtx, NULL);

    start_background_thread(NULL);

    seg_initialized = true;

    return;

error:
    seg_teardown();
    exit(EX_CONFIG);
}
