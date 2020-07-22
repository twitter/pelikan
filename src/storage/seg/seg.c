#include "seg.h"
#include "background.h"
#include "constant.h"
#include "hashtable.h"
#include "item.h"
#include "segevict.h"
#include "ttlbucket.h"
#include "datapool/datapool.h"

#include <cc_mm.h>
#include <cc_util.h>

#include <errno.h>
#include <inttypes.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

#define SEG_MODULE_NAME "storage::seg"

extern struct setting       setting;

struct seg_heapinfo         heap; /* info of all allocated segs */
struct ttl_bucket           ttl_buckets[MAX_TTL_BUCKET];

static bool                 seg_initialized = false;
seg_metrics_st              *seg_metrics = NULL;
seg_options_st              *seg_options = NULL;
seg_perttl_metrics_st       perttl[MAX_TTL_BUCKET];

proc_time_i                 flush_at = -1;
bool                        use_cas = false;
pthread_t                   bg_tid;
volatile bool               stop = false;


void
seg_print(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    log_debug("seg %" PRId32 " seg size %zu, create_at time %" PRId32
              ", ttl %" PRId32 ", evictable %u, accessible %u, %" PRIu32
              " items, write offset "
              "%" PRIu32 ", occupied size %" PRIu32 ", n_hit %" PRIu32
              ", n_hit_last %" PRIu32 ", read refcount %u, write refcount %u, "
              "prev_seg %" PRId32 ", next_seg %" PRId32,
            seg->seg_id, heap.seg_size, seg->create_at, seg->ttl,
            __atomic_load_n(&(seg->evictable), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->accessible), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->n_item), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->write_offset), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->occupied_size), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->n_hit), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->n_hit_last), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED),
            seg->prev_seg_id, seg->next_seg_id);
}


/**
 * wait until no other threads are accessing the seg (refcount == 0)
 */
static inline void
_seg_wait_refcnt(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    ASSERT(seg->accessible != 1);
    bool r_log_printed = false, w_log_printed = false;
    int r_ref, w_ref;

    w_ref = __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED);
    r_ref = __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED);

    if (w_ref) {
        log_verb("wait for seg %d refcount, current read refcount "
                 "%d, write refcount %d",
                seg_id, r_ref, w_ref);
        w_log_printed = true;
    }

    while (w_ref) {
        sched_yield();
        w_ref = __atomic_load_n(&(seg->w_refcount), __ATOMIC_RELAXED);
    }


    if (r_ref) {
        log_verb("wait for seg %d refcount, current read refcount "
                 "%d, write refcount %d",
                seg_id, r_ref, w_ref);
        r_log_printed = true;
    }

    while (r_ref) {
        sched_yield();
        r_ref = __atomic_load_n(&(seg->r_refcount), __ATOMIC_RELAXED);
    }


    if (r_log_printed || w_log_printed)
        log_verb("wait for seg %d refcount finishes", seg_id);
}


/**
 * check whether seg is accessible
 */
bool
seg_accessible(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    if (__atomic_load_n(&seg->accessible, __ATOMIC_RELAXED) == 0) {
        return false;
    }
    bool expired = seg->ttl + seg->create_at < time_proc_sec() \
            || seg->create_at <= flush_at;

    return expired;
}

bool
seg_r_ref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    if (__atomic_load_n(&seg->accessible, __ATOMIC_RELAXED) == 1) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->r_refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

void
seg_r_deref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->r_refcount, 1, __ATOMIC_RELAXED);

    ASSERT(ref >= 0);
}

bool
seg_w_ref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    if (__atomic_load_n(&seg->accessible, __ATOMIC_RELAXED) == 1) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->w_refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

void
seg_w_deref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->w_refcount, 1, __ATOMIC_RELAXED);

    ASSERT(ref >= 0);
}


/**
 * initialize the seg and seg header
 *
 * we do not use lock in this function, because the seg being initialized either
 * comes from un-allocated heap, free pool or eviction
 * in any case - the seg is only owned by current thread,
 * the one except eviction algorithm may read the seg header,
 * in order to avoid eviction algorithm picking this seg,
 * we do not clear seg->locked until it is linked into ttl_bucket
 */
static void
_seg_init(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    uint8_t *data_start = seg_get_data_start(seg_id);

    cc_memset(data_start, 0, heap.seg_size);

    seg->write_offset   = 0;
    seg->occupied_size  = 0;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    *(uint64_t *)(data_start) = SEG_MAGIC;
    seg->write_offset   = 8;
    seg->occupied_size  = 8;
#endif

    seg->prev_seg_id = -1;
    seg->next_seg_id = -1;

    seg->n_item     = 0;
    seg->n_hit      = 0;
    seg->n_hit_last = 0;

    seg->create_at = time_proc_sec();

    ASSERT(seg->accessible == 0);
    ASSERT(seg->evictable == 0);

    seg->accessible = 1;

}


static inline void
_rm_seg_from_ttl_bucket(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    struct ttl_bucket *ttl_bucket = &ttl_buckets[find_ttl_bucket_idx(seg->ttl)];

    /* all modification to seg list needs to be protected by lock */
    int status = pthread_mutex_lock(&heap.mtx);
    ASSERT(status == 0);

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

    ttl_bucket->n_seg -= 1;
    ASSERT(ttl_bucket->n_seg >= 0);

    pthread_mutex_unlock(&heap.mtx);
    log_verb("change ttl bucket seg list first %" PRId32 ", last %"PRId32 ", curr %" PRId32 " prev %" PRId32 " next %"PRId32,
            ttl_bucket->first_seg_id, ttl_bucket->last_seg_id, seg_id,
            seg->prev_seg_id, seg->next_seg_id);
}

/**
 * remove all items on this segment,
 * most of the time (common case), the seg should have no writers because
 * the eviction algorithms will avoid the segment with w_refcnt > 0 and
 * segment with next_seg_id == -1 (active segment)
 *
 * However, it is possible we are evicting a segment that is
 * actively being written to when the following happens:
 * 1. it takes too long (longer than its TTL) for the segment to
 *      finish writing and it has expired
 * 2. cache size is too small and the workload uses too many ttl buckets
 *
 *
 * because multiple threads could try to evict/expire the seg at the same time,
 * return true if current thread is able to grab the lock, otherwise false
 */
bool
seg_rm_all_item(int32_t seg_id, int expire)
{
    static char *eviction_reasons[] = {"evict", "expire"};
    struct seg *seg = &heap.segs[seg_id];
    struct item *it;

    /* prevent being picked by eviction algorithm or for merge concurrently */
    if (__atomic_exchange_n(&seg->evictable, 0, __ATOMIC_RELAXED) == 0) {
        /* this seg is either expiring or
         * being evicted by other threads if we use random eviction
         * or being merged */

        if (!expire) {
            log_warn("%s seg %" PRId32 ": seg is not evictable , ttl %" PRId32,
                      eviction_reasons[expire], seg_id, seg->ttl);

            INCR(seg_metrics, seg_evict_ex);
            return false;
        }
    };

    /* prevent future read and write access */
    __atomic_store_n(&seg->accessible, 0, __ATOMIC_RELAXED);

    /* next_seg_id == -1 indicates this is the last segment of a ttl_bucket
     * or freepool, and we should not evict the seg
     * we have tried to avoid picking such seg at eviction, but it can still
     * happen because
     * 1. this seg has been evicted and reused by another thread since it was
     *      picked by eviction algorithm (because there is no lock) - very rare
     * 2. this seg is expiring, so another thread is evicting it
     * either case should be rare, it is the effect of
     * optimistic concurrency control - no lock and roll back if needed
     *
     * since we have already "locked" the seg, it will not be held by other
     * threads, so we can check again safely
     */
    if (seg->next_seg_id == -1 && (!expire)) {
        ASSERT("this should not happen");
        __atomic_store_n(&seg->evictable, 0, __ATOMIC_SEQ_CST);

        log_warn("%s seg %" PRIu32 ": next_seg has been changed, give up",
                eviction_reasons[expire], seg_id);
        INCR(seg_metrics, seg_evict_ex);

        return false;
    }

    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    uint32_t offset = heap.seg_size -ITEM_HDR_SIZE;

    log_debug("proc time %" PRId32 ": %s seg %" PRId32 ", ttl %d",
            time_proc_sec(), eviction_reasons[expire], seg_id, seg->ttl);
    seg_print(seg_id);


#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    _rm_seg_from_ttl_bucket(seg_id);

    while (curr - seg_data < offset) {
        it = (struct item *)curr;
        if (it->klen == 0 && __atomic_load_n(&seg->n_item, __ATOMIC_SEQ_CST) == 0){
            /* no holes in the segment */
            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC || it->magic == 0);
#endif
        ASSERT(it->klen >= 0);
        ASSERT(it->vlen >= 0);
        hashtable_evict(item_key(it), it->klen, seg_id, curr - seg_data);
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
     * writing (_item_define) and insert after we clear the hashtable entries,
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
        }
    }

    /* expensive debug commands
    if (seg->n_item != 0) {
        scan_hashtable_find_seg(seg_id);
    } */

    ASSERT(seg->n_item == 0);
    ASSERT(seg->occupied_size == 0 || seg->occupied_size == 8);

    if (expire) {
        INCR(seg_metrics, seg_expire);
    } else {
        INCR(seg_metrics, seg_evict);
    }

    return true;
}


void
seg_rm_expired_seg(int32_t seg_id)
{
    bool success = seg_rm_all_item(seg_id, 1);
    if (!success) {
        return;
    }

    int status = pthread_mutex_lock(&heap.mtx);
    ASSERT(status == 0);

    seg_return_seg(seg_id);

    pthread_mutex_unlock(&heap.mtx);
}


/**
 * get a seg from free pool
 */
static inline int32_t
_seg_get_from_free_pool(void)
{
    int32_t seg_id_ret, next_seg_id;

    int status = pthread_mutex_lock(&heap.mtx);

    if (status != 0) {
        log_warn("fail to lock seg free pool");
        /* TODO(jason): clean up */
        return -1;
    }

    seg_id_ret = heap.free_seg_id;

    if (seg_id_ret == -1) {
        pthread_mutex_unlock(&heap.mtx);

        return -1;
    }

    heap.n_free_seg -= 1;
    ASSERT(heap.n_free_seg >= 0);

    next_seg_id = heap.segs[seg_id_ret].next_seg_id;
    heap.free_seg_id = next_seg_id;
    if (next_seg_id != -1) {
        heap.segs[next_seg_id].prev_seg_id = -1; /* not necessary */
    }

    ASSERT(heap.segs[seg_id_ret].write_offset == 0);

    pthread_mutex_unlock(&heap.mtx);

    return seg_id_ret;
}


/**
 * return evicted seg to free pool,
 * caller should grab the heap lock before calling this function
 **/
void
seg_return_seg(int32_t seg_id)
{
    log_debug("return seg %d to global free pool", seg_id);

    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    struct seg *seg = &heap.segs[seg_id];
    seg->next_seg_id = heap.free_seg_id;
    if (heap.free_seg_id != -1) {
        heap.segs[heap.free_seg_id].prev_seg_id = seg_id;
    }
    heap.free_seg_id = seg_id;

    /* we set all free segs as locked to prevent it being evicted
     * before finishing setup */
    ASSERT(seg->evictable == 0);
    seg->accessible = 0;

    /* this is needed to make sure the assert
     * at _seg_get_from_free_pool do not fail */
    seg->write_offset = 0;
    seg->occupied_size = 0;

    heap.n_free_seg += 1;
    log_vverb("return seg %" PRId32 " to free pool successfully", seg_id);
}


/**
 * get a new segment, we search for a free segment in the following order
 * 1. unallocated heap
 * 2. free pool
 * 3. eviction
 **/
int32_t
seg_get_new(void)
{
    /* TODO(jason): sync seg_header if we want to tolerate failures */
    evict_rstatus_e status;
    int32_t seg_id_ret;

    INCR(seg_metrics, seg_req);

    if ((seg_id_ret = _seg_get_from_free_pool()) != -1) {
        /* free pool has seg */
        log_debug("seg_get_new: allocate seg %" PRIu32 " from free pool",
                seg_id_ret);
    } else {
        /* evict one seg */
        int n_evict_retries = 0;
        while (1) {
            /* eviction may fail if other threads pick the same seg
             * (can happen in random eviction) */
            status = least_valuable_seg(&seg_id_ret);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict seg because no seg is sealed");
                INCR(seg_metrics, seg_req_ex);

                return -1;
            }
            if (seg_rm_all_item(seg_id_ret, 0)) {
                log_debug("seg_get_new: allocate seg %" PRId32 " from eviction",
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


static inline void _seg_copy(int32_t seg_id_out, int32_t seg_id_in) {

    struct item *it;
    struct seg *seg_in = &heap.segs[seg_id_in];
    struct seg *seg_out = &heap.segs[seg_id_out];
    uint8_t *seg_data_out = seg_get_data_start(seg_id_out);
    uint8_t *curr_out = seg_data_out;

    uint8_t *seg_data_in = seg_get_data_start(seg_id_in);
    uint8_t *curr_in = seg_data_in + seg_in->write_offset;
    uint32_t offset = heap.seg_size - ITEM_HDR_SIZE;

    int32_t it_sz = 0;
    bool item_up_to_date;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr_out) == SEG_MAGIC);
    curr_out += sizeof(uint64_t);
#endif

    while (curr_out - seg_data_out < offset) {
        it = (struct item *)curr_out;
        if (it->klen == 0 && it->vlen == 0){
            /* no holes in the segment */
            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC);
#endif

        ASSERT(it->klen >= 0);
        ASSERT(it->vlen >= 0);

        it_sz = item_ntotal(it);

        if (it->deleted) {
            curr_out += it_sz;
            continue;
        }

        /* first copy data */
        memcpy(curr_in, curr_out, it_sz);

        /* try to relink */
        item_up_to_date = hashtable_relink_it(item_key(it), it->klen,
                seg_id_out, curr_out - seg_data_out,
                seg_id_in, curr_in - seg_data_in);

        if (item_up_to_date) {
            curr_in += it_sz;
            __atomic_fetch_add(&seg_in->write_offset, it_sz, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_in->occupied_size, it_sz, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_in->n_item, 1, __ATOMIC_RELAXED);

            struct item *nit;
            int32_t seg_id_tmp = 0;
            nit = hashtable_get(item_key(it), it->klen, &seg_id_tmp, NULL);
            ASSERT(nit == (struct item *)(curr_in - it_sz));
        }


        curr_out += it_sz;
    }

    /* reset the unnecessary writes */
    memset(curr_in, 0, it_sz);

    log_debug("move items from seg %d to seg %d, new seg %d items, offset %d",
            seg_id_out, seg_id_in, seg_in->n_item, seg_in->write_offset);
}

/* merge two consecutive segs */
void merge_seg(int32_t seg_id1, int32_t seg_id2) {

    int accessible, evictable;
    struct seg *seg1 = &heap.segs[seg_id1];
    struct seg *seg2 = &heap.segs[seg_id2];

    ASSERT(seg1->next_seg_id == seg_id2);

    /* prevent the seg from being evicted */
    evictable = __atomic_exchange_n(&(seg1->evictable), 0, __ATOMIC_RELAXED);
    if (evictable == 0) {
        /* being evicted by another thread */
        return;
    }

    evictable = __atomic_exchange_n(&(seg2->evictable), 0, __ATOMIC_RELAXED);
    if (evictable == 0) {
        /* being evicted by another thread */
        return;
    }

    int32_t ttl_bucket_idx = find_ttl_bucket_idx(seg1->ttl);
    struct ttl_bucket *ttl_bucket = &ttl_buckets[ttl_bucket_idx];


    int32_t new_seg_id = seg_get_new();
    struct seg *new_seg = &heap.segs[new_seg_id];

    ASSERT(new_seg->evictable == 0);
    /* make sure this will not be picked for eviction */
    ASSERT(new_seg->next_seg_id == -1);

    new_seg->create_at = seg1->create_at;
    new_seg->ttl = seg1->ttl;

    _seg_copy(seg_id1, new_seg_id);
    accessible = __atomic_exchange_n(&(seg1->accessible), 0, __ATOMIC_RELAXED);
    ASSERT(accessible == 1);

    _seg_copy(seg_id2, new_seg_id);
    accessible = __atomic_exchange_n(&(seg2->accessible), 0, __ATOMIC_RELAXED);
    ASSERT(accessible == 1);

    _seg_wait_refcnt(seg_id1);
    _seg_wait_refcnt(seg_id2);

    /* change ttl bucket */
    int status = pthread_mutex_lock(&heap.mtx);
    ASSERT(status == 0);

    int32_t prev_seg_id = seg1->prev_seg_id;
    int32_t next_seg_id = seg2->next_seg_id;

    new_seg->prev_seg_id = prev_seg_id;
    new_seg->next_seg_id = next_seg_id;

    /* we should not merge the last seg */
    ASSERT(next_seg_id != -1);

    if (prev_seg_id == -1) {
        ASSERT(ttl_bucket->first_seg_id == seg_id1);

        ttl_bucket->first_seg_id = new_seg_id;
    } else {
        heap.segs[prev_seg_id].next_seg_id = new_seg_id;
    }

    heap.segs[next_seg_id].prev_seg_id = new_seg_id;

    ttl_bucket->n_seg -= 1;

    seg_return_seg(seg_id1);

    seg_return_seg(seg_id2);

    pthread_mutex_unlock(&heap.mtx);

    new_seg->evictable = 1;

    log_debug("merge seg %d and %d to seg %d in ttl bucket %d first %d last %d",
            seg_id1, seg_id2, new_seg_id, ttl_bucket_idx,
            ttl_bucket->first_seg_id, ttl_bucket->last_seg_id);
}



static void
_heap_init(void)
{
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
        pthread_mutex_lock(&heap.mtx);
        for (int32_t i = heap.max_nseg-1; i >= 0; i--) {
            heap.segs[i].seg_id = i;
            heap.segs[i].evictable = 0;
            heap.segs[i].accessible = 0;

            seg_return_seg(i);
        }
        pthread_mutex_unlock(&heap.mtx);
    }

    return CC_OK;
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

    start_background_thread(NULL);

    seg_initialized = true;

    return;

error:
    seg_teardown();
    exit(EX_CONFIG);
}
