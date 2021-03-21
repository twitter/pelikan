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
#include <stdio.h>

#ifdef USE_PMEM
#include "libpmem.h"
#endif

#define SEG_MODULE_NAME "storage::seg"

extern struct setting        setting;
extern struct seg_evict_info evict_info;
extern char                  *eviction_policy_names[];

struct seg_heapinfo heap; /* info of all allocated segs */
struct ttl_bucket   ttl_buckets[MAX_N_TTL_BUCKET];

static bool           seg_initialized = false;
seg_metrics_st        *seg_metrics    = NULL;
seg_options_st        *seg_options    = NULL;
seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];

proc_time_i   flush_at = -1;
bool use_cas = false;
pthread_t     bg_tid;
int           n_thread = 1;
volatile bool stop     = false;

static char *seg_state_change_str[] = {
    "allocation",
    "concurrent_get",
    "eviction",
    "force_eviction",
    "expiration",
    "invalid_reason",
};

void
dump_seg_info(void)
{
    struct seg *seg;
    int32_t    seg_id;

    for (int32_t i = 0; i < MAX_N_TTL_BUCKET; i++) {
        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id != -1) {
            printf("ttl bucket %4d: ", i);
        }
        else {
            continue;
        }
        while (seg_id != -1) {
            seg = &heap.segs[seg_id];
            printf("seg %d (%d), ", seg_id, seg_evictable(seg));
            seg_id = seg->next_seg_id;
        }
        printf("\n");
    }

    char         s[64];
    for (int32_t j = 0; j < heap.max_nseg; j++) {
        snprintf(s, 64, "seg %4d evictable %d", j,
            seg_evictable(&heap.segs[j]));
        SEG_PRINT(j, s, log_warn);
    }
}

/**
 * wait until no other threads are accessing the seg (refcount == 0)
 */
void
seg_wait_refcnt(int32_t seg_id)
{
    struct seg *seg          = &heap.segs[seg_id];
    ASSERT(seg->accessible != 1);
    bool       r_log_printed = false, w_log_printed = false;
    int        r_ref, w_ref;

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
seg_is_accessible(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    if (__atomic_load_n(&seg->accessible, __ATOMIC_RELAXED) == 0) {
        return false;
    }

    return seg->ttl + seg->create_at > time_proc_sec()
        && seg->create_at > flush_at;
}

bool
seg_w_ref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    if (seg_is_accessible(seg_id)) {
        __atomic_fetch_add(&seg->w_refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

void
seg_w_deref(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];

    int16_t ref = __atomic_sub_fetch(&seg->w_refcount, 1, __ATOMIC_RELAXED);

    ASSERT(ref >= 0);
}

/**
 * initialize the seg and seg header
 *
 * we do not use lock in this function, because the seg being initialized either
 * comes from un-allocated heap, free pool or eviction
 * in any case - the seg is only owned by current thread,
 * the one exception is that other threads performing evictions may
 * read the seg header,
 * in order to avoid eviction algorithm picking this seg,
 * we do not clear seg->locked until it is linked into ttl_bucket
 */
void
seg_init(int32_t seg_id)
{
    ASSERT(seg_id != -1);
    struct seg *seg = &heap.segs[seg_id];

#if defined DEBUG_MODE
    seg->seg_id_non_decr += heap.max_nseg;
    if (seg->seg_id_non_decr > 1ul << 23ul) {
        seg->seg_id_non_decr = seg->seg_id % heap.max_nseg;
    }
    seg->n_rm_item = 0;
    seg->n_rm_bytes = 0;
#endif

    uint8_t *data_start = get_seg_data_start(seg_id);

    ASSERT(seg->accessible == 0);
    ASSERT(seg->evictable == 0);

    cc_memset(data_start, 0, heap.seg_size);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    *(uint64_t *) (data_start) = SEG_MAGIC;
    seg->write_offset = 8;
    seg->live_bytes   = 8;
    seg->total_bytes  = 8;
#else
    seg->write_offset   = 0;
    seg->live_bytes  = 0;
#endif

    seg->prev_seg_id = -1;
    seg->next_seg_id = -1;

    seg->n_live_item = 0;
    seg->n_total_item = 0;

    seg->create_at = time_proc_sec();
    seg->merge_at  = 0;

    seg->accessible = 1;

    seg->n_hit         = 0;
    seg->n_active      = 0;
    seg->n_active_byte = 0;
}

void
rm_seg_from_ttl_bucket(int32_t seg_id)
{
    struct seg        *seg        = &heap.segs[seg_id];
    struct ttl_bucket *ttl_bucket = &ttl_buckets[find_ttl_bucket_idx(seg->ttl)];
    ASSERT(seg->ttl == ttl_bucket->ttl);

    /* all modification to seg chain needs to be protected by lock
     * TODO(juncheng): can change to the TTL lock? */
    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    int32_t prev_seg_id = seg->prev_seg_id;
    int32_t next_seg_id = seg->next_seg_id;

    if (prev_seg_id == -1) {
        ASSERT(ttl_bucket->first_seg_id == seg_id);

        ttl_bucket->first_seg_id = next_seg_id;
    }
    else {
        heap.segs[prev_seg_id].next_seg_id = next_seg_id;
    }

    if (next_seg_id == -1) {
        ASSERT(ttl_bucket->last_seg_id == seg_id);

        ttl_bucket->last_seg_id = prev_seg_id;
    }
    else {
        heap.segs[next_seg_id].prev_seg_id = prev_seg_id;
    }

    ttl_bucket->n_seg -= 1;
    ASSERT(ttl_bucket->n_seg >= 0);

    log_verb("remove seg %d from ttl bucket, after removal, first seg %d,"
             "last %d, prev %d, next %d", seg_id,
        ttl_bucket->first_seg_id, ttl_bucket->last_seg_id,
        seg->prev_seg_id, seg->next_seg_id);
}

/**
 * remove all items on this segment,
 * most of the time (common case), the seg should have no writers because
 * the eviction algorithms will avoid the segment with w_refcnt > 0 and
 * segment with next_seg_id == -1 (active segment)
 *
 * However, it is possible we are evicting a segment that is
 * actively being written to, when the following happens:
 * 1. it takes too long (longer than its TTL) for the segment to
 *      finish writing and it has expired
 * 2. cache size is too small and the workload uses too many ttl buckets
 *
 *
 * because multiple threads could try to evict/expire the seg at the same time,
 * return true if current thread is able to grab the lock, otherwise false
 */
/* TODO(jason): separate into two func: one lock for remove, one remove */
bool
rm_all_item_on_seg(int32_t seg_id, enum seg_state_change reason)
{
    struct seg  *seg = &heap.segs[seg_id];
    struct item *it;

    /* prevent being picked by eviction algorithm concurrently */
    if (__atomic_exchange_n(&seg->evictable, 0, __ATOMIC_RELAXED) == 0) {
        /* this seg is either expiring or being evicted by other threads */

        if (reason == SEG_EXPIRATION) {
            SEG_PRINT(seg_id, "expiring unevictable seg", log_warn);

            INCR(seg_metrics, seg_evict_ex);
        }
        return false;
    }

    /* prevent future read and write access */
    __atomic_store_n(&seg->accessible, 0, __ATOMIC_RELAXED);

    /* next_seg_id == -1 indicates this is the last segment of a ttl_bucket
     * or freepool, and we should not evict the seg
     * we have tried to avoid picking such seg at eviction, but it can still
     * happen because
     * 1. this seg has been evicted and reused by another thread since it was
     *      picked by eviction algorithm (because there is no lock) - very rare
     * 2. this seg is expiring, so another thread is removing it
     * either case should be rare, it is the effect of
     * optimistic concurrency control - no lock and roll back if needed
     *
     * since we have already "locked" the seg, it will not be held by other
     * threads, so we can check again safely
     */
    if (seg->next_seg_id == -1 &&
        reason != SEG_EXPIRATION && reason != SEG_FORCE_EVICTION) {
        /* "this should not happen" */
        ASSERT(0);
//        __atomic_store_n(&seg->evictable, 0, __ATOMIC_SEQ_CST);
//        INCR(seg_metrics, seg_evict_ex);

        return false;
    }

    uint8_t  *seg_data = get_seg_data_start(seg_id);
    uint8_t  *curr     = seg_data;
    uint32_t offset    = MIN(seg->write_offset, heap.seg_size) - ITEM_HDR_SIZE;

    SEG_PRINT(seg_id, seg_state_change_str[reason], log_debug);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *) (curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    /* remove segment from TTL bucket */
    pthread_mutex_lock(&heap.mtx);
    rm_seg_from_ttl_bucket(seg_id);
    pthread_mutex_unlock(&heap.mtx);

    while (curr - seg_data < offset) {
        /* check both offset and n_live_item is because when a segment is expiring
         * and have a slow writer on it, we could observe n_live_item == 0,
         * but we haven't reached offset */
        it = (struct item *) curr;
        if (seg->n_live_item == 0) {
            ASSERT(seg->live_bytes == 0 || seg->live_bytes == 8);

            break;
        }
        if (it->klen == 0 && it->vlen == 0) {
#if defined(CC_ASSERT_PANIC) && defined(DEBUG_MODE)
            scan_hashtable_find_seg(seg->seg_id_non_decr);
#endif
            ASSERT(__atomic_load_n(&seg->n_live_item, __ATOMIC_SEQ_CST) == 0);

            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC);
#endif
        ASSERT(it->klen > 0);
        ASSERT(it->vlen >= 0);

#if defined DEBUG_MODE
        hashtable_evict(item_key(it), it->klen, seg->seg_id_non_decr,
            curr - seg_data);
#else
        hashtable_evict(item_key(it), it->klen, seg->seg_id, curr - seg_data);
#endif

        ASSERT(seg->n_live_item >= 0);
        ASSERT(seg->live_bytes >= 0);

        curr += item_ntotal(it);
    }

    /* at this point, seg->n_live_item could be negative
     * if it is an expired segment and a new item is being wriiten very slowly,
     * and not inserted into hash table */
//    ASSERT(__atomic_load_n(&seg->n_live_item, __ATOMIC_ACQUIRE) >= 0);

    /* all operation up till here does not require refcount to be 0
     * because the data on the segment is not cleared yet,
     * now we are ready to clear the segment data, we need to check refcount.
     * Because we have already locked the segment before removing entries
     * from hashtable, ideally by the time we have removed all hashtable
     * entries, all previous requests on this segment have all finished */
    seg_wait_refcnt(seg_id);

    /* optimistic concurrency control:
     * because we didn't wait for refcount before remove hashtable entries
     * it is possible that there are some very slow writers, which finish
     * writing (_item_define) and insert after we clear the hashtable entries,
     * so we need to double check, in most cases, this should not happen */

    if (__atomic_load_n(&seg->n_live_item, __ATOMIC_SEQ_CST) > 0) {
        INCR(seg_metrics, seg_evict_retry);
        /* because we don't know which item is newly written, so we
         * have to remove all items again */
        curr = seg_data;
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        curr += sizeof(uint64_t);
#endif
        while (curr - seg_data < offset) {
            it = (struct item *) curr;
#if defined DEBUG_MODE
            hashtable_evict(item_key(it), it->klen, seg->seg_id_non_decr,
                curr - seg_data);
#else
            hashtable_evict(item_key(it), it->klen, seg->seg_id, curr - seg_data);
#endif
            curr += item_ntotal(it);
        }
    }

    /* expensive debug commands */
    if (seg->n_live_item != 0) {
        log_warn("removed all items from segment, but %d items left",
            seg->n_live_item);
#if defined(CC_ASSERT_PANIC) && defined(DEBUG_MODE)
        scan_hashtable_find_seg(heap.segs[seg_id].seg_id_non_decr);
#endif
    }

    ASSERT(seg->n_live_item == 0);
    ASSERT(seg->live_bytes == 0 || seg->live_bytes == 8);

    return true;
}

rstatus_i
expire_seg(int32_t seg_id)
{
    bool success = rm_all_item_on_seg(seg_id, SEG_EXPIRATION);
    if (!success) {
        return CC_ERROR;
    }

    int status = pthread_mutex_lock(&heap.mtx);
    ASSERT(status == 0);

    seg_add_to_freepool(seg_id, SEG_EXPIRATION);

    pthread_mutex_unlock(&heap.mtx);

    INCR(seg_metrics, seg_expire);

    return CC_OK;
}

/**
 * get a seg from free pool,
 *
 * use_reserved: merge-based eviction reserves one seg per thread
 * return the segment id if there are free segment, -1 if not
 */
int32_t
seg_get_from_freepool(bool use_reserved)
{
    int32_t seg_id_ret, next_seg_id;

    int status = pthread_mutex_lock(&heap.mtx);

    if (status != 0) {
        log_warn("fail to lock seg free pool");
        pthread_mutex_unlock(&heap.mtx);

        return -1;
    }

    if (heap.n_free_seg == 0 ||
        (!use_reserved && heap.n_free_seg <= heap.n_reserved_seg)) {
        pthread_mutex_unlock(&heap.mtx);

        return -1;
    }

    heap.n_free_seg -= 1;
    ASSERT(heap.n_free_seg >= 0);

    seg_id_ret = heap.free_seg_id;
    ASSERT(seg_id_ret >= 0);

    next_seg_id = heap.segs[seg_id_ret].next_seg_id;
    heap.free_seg_id = next_seg_id;
    if (next_seg_id != -1) {
        heap.segs[next_seg_id].prev_seg_id = -1;
    }

    ASSERT(heap.segs[seg_id_ret].write_offset == 0);

    pthread_mutex_unlock(&heap.mtx);

    return seg_id_ret;
}

/**
 * add evicted/allocated seg to free pool,
 * caller should grab the heap lock before calling this function
 **/
void
seg_add_to_freepool(int32_t seg_id, enum seg_state_change reason)
{
    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    struct seg *seg = &heap.segs[seg_id];
    seg->next_seg_id = heap.free_seg_id;
    seg->prev_seg_id = -1;
    if (heap.free_seg_id != -1) {
        ASSERT(heap.segs[heap.free_seg_id].prev_seg_id == -1);
        heap.segs[heap.free_seg_id].prev_seg_id = seg_id;
    }
    heap.free_seg_id = seg_id;

    /* we set all free segs as locked to prevent it being evicted
     * before finishing setup */
    ASSERT(seg->evictable == 0);
    seg->accessible = 0;

    /* this is needed to make sure the assert
     * at seg_get_from_freepool do not fail */
    seg->write_offset = 0;
    seg->live_bytes   = 0;

    heap.n_free_seg += 1;

    log_vverb("add %s seg %d to free pool, %d free segs",
        seg_state_change_str[reason], seg_id, heap.n_free_seg);
}

/**
 * get a new segment, search for a free segment in the following order
 * 1. unallocated heap
 * 2. free pool
 * 3. eviction
 **/
int32_t
seg_get_new(void)
{
#define MAX_RETRIES 8
    evict_rstatus_e status;
    int32_t         seg_id_ret;
    /* eviction may fail if other threads pick the same seg */
    int             n_retries_left = MAX_RETRIES;

    INCR(seg_metrics, seg_get);

    seg_id_ret = seg_get_from_freepool(false);

    while (seg_id_ret == -1 && n_retries_left >= 0) {
        /* evict seg */
        if (evict_info.policy == EVICT_MERGE_FIFO) {
            status = seg_merge_evict(&seg_id_ret);
        } else {
            status = seg_evict(&seg_id_ret);
        }

        if (status == EVICT_OK) {
            break;
        }

        if (--n_retries_left < MAX_RETRIES) {
            log_warn("retry %d", n_retries_left);

            INCR(seg_metrics, seg_evict_retry);
        }
    }

    if (seg_id_ret == -1) {
        INCR(seg_metrics, seg_get_ex);
        log_error("unable to get new seg from eviction");

        return -1;
    }

    seg_init(seg_id_ret);

    return seg_id_ret;
}

static void
heap_init(void)
{
    heap.max_nseg  = heap.heap_size / heap.seg_size;
    heap.heap_size = heap.max_nseg * heap.seg_size;
    heap.base      = NULL;

    if (!heap.prealloc) {
        log_crit("%s only support prealloc", SEG_MODULE_NAME);
        exit(EX_CONFIG);
    }
}

static int
setup_heap_mem(void)
{
    int datapool_fresh = 1;

    heap.pool = datapool_open(heap.poolpath, heap.poolname, heap.heap_size,
        &datapool_fresh, heap.prefault);

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
seg_heap_setup(void)
{
    heap_init();

    int    dram_fresh = 1;
    size_t seg_hdr_sz = SEG_HDR_SIZE * heap.max_nseg;

    dram_fresh = setup_heap_mem();
    pthread_mutex_init(&heap.mtx, NULL);

    heap.segs = cc_zalloc(seg_hdr_sz);

    if (!dram_fresh) {
        /* TODO(jason): recover */
        ;
    }
    else {
        pthread_mutex_lock(&heap.mtx);
        heap.n_free_seg = 0;
        for (int32_t i = heap.max_nseg - 1; i >= 0; i--) {
            heap.segs[i].seg_id          = i;
#ifdef DEBUG_MODE
            heap.segs[i].seg_id_non_decr = i;
#endif
            heap.segs[i].evictable       = 0;
            heap.segs[i].accessible      = 0;

            seg_add_to_freepool(i, SEG_ALLOCATION);
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

    flush_at        = -1;
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

    seg_metrics = metrics;

    if (options == NULL) {
        log_crit("no option is provided for seg initialization");
        exit(EX_CONFIG);
    }

    flush_at = -1;
    stop     = false;

    seg_options = options;
    n_thread    = option_uint(&seg_options->seg_n_thread);

    heap.seg_size  = option_uint(&seg_options->seg_size);
    heap.heap_size = option_uint(&seg_options->heap_mem);
    log_verb("cache size %" PRIu64, heap.heap_size);

    heap.free_seg_id = -1;
    heap.prealloc    = option_bool(&seg_options->seg_prealloc);
    heap.prefault    = option_bool(&seg_options->datapool_prefault);

    heap.poolpath = option_str(&seg_options->datapool_path);
    heap.poolname = option_str(&seg_options->datapool_name);

    heap.n_reserved_seg = 0;

    use_cas = option_bool(&seg_options->seg_use_cas);

    hashtable_setup(option_uint(&seg_options->hash_power));

    if (seg_heap_setup() != CC_OK) {
        log_crit("Could not setup seg heap info");
        goto error;
    }

    ttl_bucket_setup();

    evict_info.merge_opt.seg_n_merge     =
        option_uint(&seg_options->seg_n_merge);
    evict_info.merge_opt.seg_n_max_merge =
        option_uint(&seg_options->seg_n_max_merge);
    segevict_setup(option_uint(&options->seg_evict_opt),
        option_uint(&seg_options->seg_mature_time));
    if (evict_info.policy == EVICT_MERGE_FIFO) {
        heap.n_reserved_seg = n_thread;
    }

    start_background_thread(NULL);

    seg_initialized = true;

    log_info("Seg header size: %d, item header size: %d, eviction algorithm %s",
        SEG_HDR_SIZE, ITEM_HDR_SIZE, eviction_policy_names[evict_info.policy]);

    return;

    error:
    seg_teardown();
    exit(EX_CONFIG);
}
