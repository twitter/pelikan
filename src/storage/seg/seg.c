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

#define SEG_MODULE_NAME "storage::seg"

extern struct setting       setting;

struct seg_heapinfo         heap; /* info of all allocated segs */
struct ttl_bucket           ttl_buckets[MAX_N_TTL_BUCKET];

static bool                 seg_initialized = false;
seg_metrics_st              *seg_metrics = NULL;
seg_options_st              *seg_options = NULL;
seg_perttl_metrics_st       perttl[MAX_N_TTL_BUCKET];

proc_time_i                 flush_at = -1;
bool                        use_cas = false;
pthread_t                   bg_tid;
volatile bool               stop = false;

static int64_t merge_epoch = 1;
static int min_mature_time = 5;


static inline void
prep_seg_to_merge(int32_t start_seg_id, struct seg *segs_to_merge[],
        int *n_seg_to_merge, double *merge_keep_ratio);
int32_t merge_segs(struct seg *segs_to_merge[], int32_t n_seg_most);


void seg_print(int32_t seg_id) {
    struct seg *st = &heap.segs[seg_id];
    log_debug("seg %" PRId32 " seg size %zu, create_at time %" PRId32
              ", merge at %" PRId32 ", age %d"
              ", ttl %" PRId32 ", evictable %u, accessible %u, "
              "write offset %" PRId32 ", occupied size %" PRId32
              ", %" PRId32 " items , n_hit %" PRId32
              ", n_hit_last %" PRId32 ", read refcount %d, write refcount %d, "
              "prev_seg %" PRId32 ", next_seg %" PRId32,
            st->seg_id, heap.seg_size, st->create_at, st->merge_at,
            st->merge_at > 0 ? time_proc_sec() - st->merge_at : time_proc_sec() - st->create_at,
            st->ttl,
            __atomic_load_n(&(st->evictable), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->accessible), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->write_offset), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->occupied_size), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_item), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_hit), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_hit_last), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->w_refcount), __ATOMIC_RELAXED),
            st->prev_seg_id, st->next_seg_id);
}

void seg_print_warn(int32_t seg_id) {
    struct seg *st = &heap.segs[seg_id];
    log_warn("seg %" PRId32 " seg size %zu, create_at time %" PRId32
            ", merge at %" PRId32 ", age %d"
            ", ttl %" PRId32 ", evictable %u, accessible %u, "
                             "write offset %" PRId32 ", occupied size %" PRId32
            ", %" PRId32 " items , n_hit %" PRId32
            ", n_hit_last %" PRId32 ", read refcount %d, write refcount %d, "
                                    "prev_seg %" PRId32 ", next_seg %" PRId32,
            st->seg_id, heap.seg_size, st->create_at, st->merge_at,
            st->merge_at > 0 ? time_proc_sec() - st->merge_at : time_proc_sec() - st->create_at,
            st->ttl,
            __atomic_load_n(&(st->evictable), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->accessible), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->write_offset), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->occupied_size), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_item), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_hit), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->n_hit_last), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->r_refcount), __ATOMIC_RELAXED),
            __atomic_load_n(&(st->w_refcount), __ATOMIC_RELAXED),
            st->prev_seg_id, st->next_seg_id);
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

    return !expired;
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

    int16_t ref = __atomic_sub_fetch(&seg->r_refcount, 1, __ATOMIC_RELAXED);

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

    int16_t ref = __atomic_sub_fetch(&seg->w_refcount, 1, __ATOMIC_RELAXED);

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
    ASSERT(seg_id != -1);
    struct seg *seg = &heap.segs[seg_id];
    uint8_t *data_start = seg_get_data_start(seg_id);

    /* I think we still need this to prevent incorrect data
     * when evicting/expiring a segment that is waiting for a super super
     * long write
     **/
//    cc_memset(data_start, 0, heap.seg_size);
    cc_memset(data_start + heap.seg_size / 2, 0, heap.seg_size / 2);

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

    seg->create_at  = time_proc_sec();
    seg->merge_at   = 0;

    ASSERT(seg->accessible == 0);
    ASSERT(seg->evictable == 0);

    seg->accessible = 1;

#ifdef TRACK_ADVANCED_STAT
    seg->last_merge_epoch = merge_epoch;
    seg->n_active = 0;
    seg->n_active_byte = 0;
    memset(seg->active_obj, 0, sizeof(uint16_t) * 131072);
#endif
}


static inline void
_rm_seg_from_ttl_bucket(int32_t seg_id)
{
    struct seg *seg = &heap.segs[seg_id];
    struct ttl_bucket *ttl_bucket = &ttl_buckets[find_ttl_bucket_idx(seg->ttl)];
    ASSERT(seg->ttl == ttl_bucket->ttl);

    /* all modification to seg list needs to be protected by lock */
    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

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

    log_verb("change ttl bucket seg list first %" PRId32 ", last %"PRId32
             ", curr %" PRId32 " prev %" PRId32 " next %"PRId32,
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
         * being evicted by other threads if we use random eviction */

        if (!expire) {
            log_warn("%s seg %" PRId32 ": seg is not evictable, next seg %d"
                     " ttl %" PRId32,
                      eviction_reasons[expire], seg_id, seg->next_seg_id,
                    seg->ttl);
            seg_print(seg_id);

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
     * 2. this seg is expiring, so another thread is evicting it
     * either case should be rare, it is the effect of
     * optimistic concurrency control - no lock and roll back if needed
     *
     * since we have already "locked" the seg, it will not be held by other
     * threads, so we can check again safely
     */
    if (seg->next_seg_id == -1 && (!expire)) {
        /* "this should not happen" */
        ASSERT(0);
        __atomic_store_n(&seg->evictable, 0, __ATOMIC_SEQ_CST);

        log_warn("%s seg %" PRIu32 ": next_seg has been changed, give up",
                eviction_reasons[expire], seg_id);
        INCR(seg_metrics, seg_evict_ex);

        return false;
    }

    uint8_t *seg_data = seg_get_data_start(seg_id);
    uint8_t *curr = seg_data;
    uint32_t offset = MIN(seg->write_offset, heap.seg_size -ITEM_HDR_SIZE);

    log_debug("proc time %" PRId32 ": %s seg %" PRId32 ", ttl %d",
            time_proc_sec(), eviction_reasons[expire], seg_id, seg->ttl);
    seg_print(seg_id);


#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
    curr += sizeof(uint64_t);
#endif

    pthread_mutex_lock(&heap.mtx);
    _rm_seg_from_ttl_bucket(seg_id);
    pthread_mutex_unlock(&heap.mtx);

    while (curr - seg_data < offset) {
        /* check both offset and n_item is because when a segment is expiring
         * and have a slow writer on it, we could observe n_item == 0,
         * but we haven't reached offset,
         * this is the consequence of not memset the seg data to be 0 */
        it = (struct item *)curr;
        if (it->klen == 0 && __atomic_load_n(&seg->n_item, __ATOMIC_SEQ_CST) == 0){
            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC || it->magic == 0);
#endif
        ASSERT(it->klen >= 0);
        ASSERT(it->vlen >= 0);

        if (!it->deleted)
            hashtable_evict(item_key(it), it->klen, seg_id, curr - seg_data);
        curr += item_ntotal(it);
    }

    ASSERT(__atomic_load_n(&seg->n_item, __ATOMIC_ACQUIRE) >= 0);

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


rstatus_i
seg_rm_expired_seg(int32_t seg_id)
{
    bool success = seg_rm_all_item(seg_id, 1);
    if (!success) {
        return CC_ERROR;
    }

    int status = pthread_mutex_lock(&heap.mtx);
    ASSERT(status == 0);

    seg_return_seg(seg_id);

    pthread_mutex_unlock(&heap.mtx);

    return CC_OK;
}


/**
 * get a seg from free pool
 */
static inline int32_t
_seg_get_from_free_pool(bool use_reserved)
{
    int32_t seg_id_ret, next_seg_id;

    int status = pthread_mutex_lock(&heap.mtx);

    if (status != 0) {
        log_warn("fail to lock seg free pool");
        /* TODO(jason): clean up */
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
        heap.segs[next_seg_id].prev_seg_id = -1; /* not necessary */
    }

    ASSERT(heap.segs[seg_id_ret].write_offset == 0);

    pthread_mutex_unlock(&heap.mtx);

    return seg_id_ret;
}


#ifdef USE_MERGE
static inline bool
_check_merge_seg2(void) {
    struct seg *seg = NULL, *next1_seg, *next2_seg = NULL;

    int32_t ttl_bkt_idx;
    struct ttl_bucket *ttl_bkt;
    int i;
    bool found = false;

    static __thread int32_t last_ttl_bkt_idx = 0;
    static __thread proc_time_i last_round_sec = 0;
    static __thread delta_time_i merge_epoch_sec = 0;

    static __thread struct seg *segs_to_merge[N_MAX_SEG_MERGE];
    static __thread double merge_keep_ratio[N_MAX_SEG_MERGE];
    static __thread int32_t at_most_n_seg;

    if (heap.n_free_seg > 8) {
        return false;
    }

test:
    /* it is important to have MAX_N_TTL_BUCKET+1, because
     * if there is only one TTL bucket, we need to check this
     * ttl bucket again after reaching the end of bucket */
    for (i = 0; i < MAX_N_TTL_BUCKET+1; i++) {
        ttl_bkt_idx = (last_ttl_bkt_idx + i) % MAX_N_TTL_BUCKET;
        ttl_bkt = &ttl_buckets[ttl_bkt_idx];
        if (ttl_bkt_idx == 0) {
            if (last_round_sec != 0) {
                merge_epoch_sec = time_proc_sec() - last_round_sec;
//                if (merge_epoch_sec > min_mature_time * 2) {
//                    min_mature_time = (int) (min_mature_time + (merge_epoch_sec-min_mature_time)/8);
//                } else if (min_mature_time * 2 > merge_epoch_sec) {
//                    min_mature_time = min_mature_time >> 2;
//                }
                log_info("*************** epoch %d mature %d\n",
                        merge_epoch_sec, min_mature_time);
            }

            last_round_sec = time_proc_sec();
        }

        if (ttl_buckets[ttl_bkt_idx].first_seg_id == -1)
            continue;

        if (pthread_mutex_trylock(&ttl_bkt->mtx) != 0) {
            /* with more than 16 threads and 20% write, this lock becomes
             * the bottleneck, so for scalability, we just check next TTL bucket
             */
            continue;
        }

        if (ttl_buckets[ttl_bkt_idx].next_seg_to_merge != -1) {
            seg = &heap.segs[ttl_buckets[ttl_bkt_idx].next_seg_to_merge];
        } else {
            seg = &heap.segs[ttl_buckets[ttl_bkt_idx].first_seg_id];
        }

        while (1) {
            if (seg->next_seg_id == -1) {
                break;
            }
            next1_seg = &heap.segs[seg->next_seg_id];

            if (next1_seg->next_seg_id == -1) {
                break;
            }
            next2_seg = &heap.segs[next1_seg->next_seg_id];

            if (next2_seg->next_seg_id == -1) {
                break;
            }

            if (seg_mergeable(seg)) {
                if (seg_mergeable(next1_seg)) {
                    if (seg_mergeable(next2_seg)) {
                        found = true;
                        break;
                    } else {
                        if (next2_seg->next_seg_id != -1) {
                            seg = &heap.segs[next2_seg->next_seg_id];
                            continue;
                        } else {
                            break;
                        }
                    }
                } else {
                    seg = next2_seg;
                    continue;
                }
            } else {
                seg = next1_seg;
                continue;
            }
        }


        if (!found) {
            ttl_buckets[ttl_bkt_idx].next_seg_to_merge = -1;
            int32_t seg_id = ttl_buckets[ttl_bkt_idx].first_seg_id;
            delta_time_i first_seg_age = time_proc_sec() -
                    heap.segs[seg_id].create_at;
            /* the segments in this bucket cannot be merged, but it has been
             * too old, we evict it */
            if (merge_epoch_sec > 0 && first_seg_age > merge_epoch_sec * N_SEG_MERGE) {
                seg_rm_all_item(seg_id, true);
                pthread_mutex_lock(&heap.mtx);
                seg_return_seg(seg_id);
                pthread_mutex_unlock(&heap.mtx);
                last_ttl_bkt_idx = ttl_bkt_idx + 1;
                pthread_mutex_unlock(&ttl_bkt->mtx);
                return true;
            }
            /* next ttl bucket please */
            pthread_mutex_unlock(&ttl_bkt->mtx);
            continue;
        }


        /* block the eviction of next N_MAX_SEG_MERGE segments */
        prep_seg_to_merge(seg->seg_id, segs_to_merge, &at_most_n_seg, merge_keep_ratio);
        pthread_mutex_unlock(&ttl_bkt->mtx);

        /* I hope I can move the ttl_bkt lock out of merge_segs
         * it is not clear when it is in the merge_segs */
        ttl_buckets[ttl_bkt_idx].next_seg_to_merge = merge_segs(segs_to_merge, at_most_n_seg);
        last_ttl_bkt_idx = ttl_bkt_idx;


        return true;
    }

    for (int j=0; j<heap.max_nseg; j++) {
        seg_print_warn(j);
        log_warn("%d mergeable %d", j, seg_mergeable(&heap.segs[j]));
    }
//
//    for (i = 0; i < MAX_N_TTL_BUCKET+1; i++) {
//        if (ttl_buckets[i].first_seg_id == -1)
//            continue;
//        seg = &heap.segs[ttl_buckets[i].first_seg_id];
//        while (seg != NULL) {
//            printf("seg %d (%d), ", seg->seg_id, seg_mergeable(seg));
//            if (seg->next_seg_id != -1)
//                seg = &heap.segs[seg->next_seg_id];
//            else
//                seg = NULL;
//        }
//        printf("\n");
//    }
//    goto test;
    ASSERT(0);
    return false;
}

static inline bool _check_merge_seg(void) {
    return _check_merge_seg2();
}

int32_t seg_get_new_with_merge(void) {
//    static proc_time_i last_merge = 0, last_clear = 0;
//    if (last_merge == 0) {
//        last_merge = time_proc_sec();
//        last_clear = time_proc_sec();
//    }

    int32_t seg_id_ret;

    while ((seg_id_ret = _seg_get_from_free_pool(false)) == -1) {
        if (!_check_merge_seg())
            /* better evict a random one */
            return -1;
    }
    _seg_init(seg_id_ret);

    return seg_id_ret;
}
#endif


/**
 * return evicted seg to free pool,
 * caller should grab the heap lock before calling this function
 **/
void
seg_return_seg(int32_t seg_id)
{
    log_vverb("return seg %d to free pool", seg_id);

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
     * at _seg_get_from_free_pool do not fail */
    seg->write_offset = 0;
    seg->occupied_size = 0;

    heap.n_free_seg += 1;
    log_verb("return seg %d to free pool, %d free segs",
            seg_id, heap.n_free_seg);
}


/**
 * get a new segment, we search for a free segment in the following order
 * 1. unallocated heap
 * 2. free pool
 * 3. eviction
 **/
int32_t
seg_get_new_no_merge(void)
{
    evict_rstatus_e status;
    int32_t seg_id_ret;

    INCR(seg_metrics, seg_req);

    if ((seg_id_ret = _seg_get_from_free_pool(true)) != -1) {
        /* free pool has seg */
        log_verb("seg_get_new: allocate seg %" PRId32 " from free pool",
                seg_id_ret);
    } else {
        /* evict one seg */
        int n_evict_retries = 0;
        while (1) {
            /* eviction may fail if other threads pick the same seg
             * (can happen in random eviction) */
            status = least_valuable_seg(&seg_id_ret);
            if (status == EVICT_NO_SEALED_SEG) {
                log_warn("unable to evict seg because no seg can be evicted");
                INCR(seg_metrics, seg_req_ex);

                return -1;
            }

            if (seg_rm_all_item(seg_id_ret, 0)) {
                log_verb("seg_get_new: allocate seg %" PRId32 " from eviction",
                        seg_id_ret);
                break;
            }

            if (++n_evict_retries >= 8) {
                for (int x=0; x<heap.max_nseg; x++) {
                    seg_print_warn(x);
                }
                log_error("seg_get_new: unable to evict after 8 retries");
                return -1;
            }
        }
    }

    _seg_init(seg_id_ret);

    return seg_id_ret;
}

int32_t seg_get_new(void) {
#ifdef USE_MERGE
    return seg_get_new_with_merge();
#else
    return seg_get_new_no_merge();
#endif
}


#ifdef USE_MERGE
static inline void _seg_copy(int32_t seg_id_dest, int32_t seg_id_src,
        double *cutoff_freq, double target_ratio) {
    struct item *it;
    struct item *last_it = NULL;
    struct seg *seg_dest = &heap.segs[seg_id_dest];
    struct seg *seg_src = &heap.segs[seg_id_src];
    uint8_t *seg_data_src = seg_get_data_start(seg_id_src);
    uint8_t *curr_src = seg_data_src;

    uint8_t *seg_data_dest = seg_get_data_start(seg_id_dest);
    uint32_t offset = MIN(seg_src->write_offset, heap.seg_size - ITEM_HDR_SIZE);

    int32_t it_sz = 0;
    bool item_up_to_date;
    bool seg_in_full = false;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(seg_data_dest) == SEG_MAGIC);
    ASSERT(*(uint64_t *)(curr_src) == SEG_MAGIC);
    curr_src += sizeof(uint64_t);
#endif

    int n_scanned = 0, n_copied = 0;
    double mean_size = (double) seg_src->write_offset / seg_src->n_item;
    double cutoff = (1 + *cutoff_freq) / 2;
//    double cutoff = *cutoff_freq;
//    cutoff = 1;
    int update_intvl = (int) heap.seg_size / 10;
    int n_th_update = 1;

    double hit;
    bool copy_all_items = false;

    while (curr_src - seg_data_src < offset) {
        last_it = it;
        it = (struct item *) curr_src;

        ASSERT(seg_src->n_item >= 0);

        if (it->klen == 0 && it->vlen == 0){
            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC);
#endif

        it_sz = item_ntotal(it);
        n_scanned += it_sz;
        if (n_scanned >= n_th_update * update_intvl) {
            n_th_update += 1;
            double t = (double) n_copied/n_scanned - target_ratio;
            if (t > 0.1 || t < -0.1) {
                cutoff = cutoff * (1 + t);
            }
        }

        /* we will not merge a new segment, so let's copy all items left,
         * most of the time, the impact of this is small */
        if (!copy_all_items && (seg_dest->write_offset >= SEG_MERGE_MARGIN)
                && curr_src - seg_data_src > SEG_MERGE_MARGIN) {
            copy_all_items = true;
            log_verb("set copy %d %d/%d, last item sz %d", seg_id_src, curr_src-seg_data_src,
                    seg_dest->write_offset, item_ntotal(last_it));
        }

        if (it->deleted) {
            curr_src += it_sz;
            continue;
        }

#ifdef TRACK_ADVANCED_STAT
        hit = seg_src->active_obj[(curr_src - seg_data_src) >> 3u];
#else
        hit = hashtable_get_it_freq(item_key(it), it->klen, seg_id_src, curr_src - seg_data_src);
#endif
        hit = (double) hit / ((double) it_sz/mean_size);

        if (hit <= cutoff && (!copy_all_items)) {
            hashtable_evict(item_key(it), it->klen, seg_id_src, curr_src - seg_data_src);
            curr_src += it_sz;
            continue;
        }

        if (seg_dest->write_offset + it_sz > heap.seg_size) {
            /* TODO(jason): add a new metric */
            if (!seg_in_full) {
                seg_in_full = true;
                log_debug("copy from seg %" PRId32 " to seg %" PRId32
                         ", destination seg full %d + %d src offset %d",
                        seg_id_src, seg_id_dest, seg_dest->write_offset, it_sz,
                        curr_src - seg_data_src);
            }

            hashtable_evict(item_key(it), it->klen, seg_id_src, curr_src - seg_data_src);
            curr_src += it_sz;
            continue;
        }

#ifdef REAL_COPY
        /* first copy data */
        memcpy(seg_data_dest + seg_dest->write_offset, curr_src, it_sz);
#else
        memcpy(seg_data_dest + seg_dest->write_offset, curr_src, ITEM_HDR_SIZE + it->klen);
#endif

        /* try to relink */
        item_up_to_date = hashtable_relink_it(item_key(it), it->klen,
                seg_id_src, curr_src - seg_data_src, seg_id_dest,
                seg_dest->write_offset);

        if (item_up_to_date) {
            seg_dest->write_offset += it_sz;
            seg_dest->occupied_size += it_sz;
            seg_dest->n_item += 1;
            seg_src->n_item -= 1;
            n_copied += it_sz;
        }

        curr_src += it_sz;
    }

    *cutoff_freq = cutoff;
    log_debug("move items from seg %d to seg %d, new seg %d items, offset %d, cutoff %d, ",
            seg_id_src, seg_id_dest, seg_dest->n_item, seg_dest->write_offset,
            *cutoff_freq);
}

bool seg_mergeable(struct seg *seg) {
    if (seg == NULL)
        return false;
    bool is_mergeable;
    is_mergeable = seg->evictable == 1;
    is_mergeable = is_mergeable && (seg->next_seg_id != -1);
    /* a magic number - we don't want to merge just created seg */
    /* TODO(jason): 600 needs to be adaptive */
    is_mergeable = is_mergeable && time_proc_sec() - seg->create_at >= min_mature_time;
    /* don't merge segments that will expire soon */
    is_mergeable = is_mergeable &&
            seg->create_at + seg->ttl - time_proc_sec() > 20;
    return is_mergeable;
}

/**
 * lock at most N_MAX_SEG_MERGE segments to prevent other threads evicting
 */
static inline void prep_seg_to_merge(int32_t start_seg_id,
        struct seg *segs_to_merge[], int *n_seg_to_merge,
        double *merge_keep_ratio) {

    *n_seg_to_merge = 0;
    int32_t curr_seg_id = start_seg_id;
    struct seg *curr_seg;

    uint8_t evictable;
    int32_t n_active_sum = 0;

    pthread_mutex_lock(&heap.mtx);
    for (int i = 0; i < N_MAX_SEG_MERGE; i++) {
        if (curr_seg_id == -1) {
            /* this could happen when prev seg is evicted */
            break;
        }
        curr_seg = &heap.segs[curr_seg_id];
        if (!seg_mergeable(curr_seg)) {
            curr_seg_id = curr_seg->next_seg_id;
            continue;
        }
        evictable = __atomic_exchange_n(&curr_seg->evictable, 0,
                __ATOMIC_RELAXED);
        if (evictable == 0) {
            /* concurrent merge and evict */
            curr_seg_id = curr_seg->next_seg_id;
            continue;
        }
        segs_to_merge[(*n_seg_to_merge)++] = curr_seg;
        curr_seg_id = curr_seg->next_seg_id;
    }
    pthread_mutex_unlock(&heap.mtx);

    for (int i = 0; i < N_MAX_SEG_MERGE; i++) {
        merge_keep_ratio[i] = merge_keep_ratio[i] / n_active_sum;
    }

    ASSERT(*n_seg_to_merge > 1);
}


static inline void
_replace_seg_in_chain(int32_t new_seg_id, int32_t old_seg_id)
{
    struct seg *new_seg = &heap.segs[new_seg_id];
    struct seg *old_seg = &heap.segs[old_seg_id];
    struct ttl_bucket *tb = &ttl_buckets[find_ttl_bucket_idx(old_seg->ttl)];

    /* all modification to seg list needs to be protected by lock */
    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    int32_t prev_seg_id = old_seg->prev_seg_id;
    int32_t next_seg_id = old_seg->next_seg_id;

    if (prev_seg_id == -1) {
        ASSERT(tb->first_seg_id == old_seg_id);

        tb->first_seg_id = new_seg_id;
    } else {
        heap.segs[prev_seg_id].next_seg_id = new_seg_id;
    }

    ASSERT(next_seg_id != -1);
        heap.segs[next_seg_id].prev_seg_id = new_seg_id;

    new_seg->prev_seg_id = prev_seg_id;
    new_seg->next_seg_id = next_seg_id;
}


/* merge at most n_seg consecutive segs into one seg,
 * if the merged seg is full return earlier
 *
 * the return value indicates how many segs are merged
 *
 **/
int32_t merge_segs(struct seg *segs_to_merge[], int at_most_n_seg) {

    int32_t curr_seg_id;
    struct seg *curr_seg;
    uint8_t accessible;
    int n_merged = 0;

    /* this is the next seg_id of the last segment, we keep copy of it
     * in case there are no active objects in all these segments,
     * this is the return value */
    int32_t last_seg_next_seg_id = segs_to_merge[at_most_n_seg-1]->next_seg_id;

    /* prepare new seg */
    int32_t new_seg_id = _seg_get_from_free_pool(true);
    _seg_init(new_seg_id);

    struct seg *new_seg = &heap.segs[new_seg_id];
    ASSERT(new_seg->evictable == 0);


    new_seg->create_at = segs_to_merge[0]->create_at;
    new_seg->merge_at = time_proc_sec();
    new_seg->ttl = segs_to_merge[0]->ttl;
    new_seg->accessible = 1;
    new_seg->prev_seg_id = segs_to_merge[0]->prev_seg_id;
    double cutoff_freq = 1;

    /* start from start_seg until new_seg is full or no seg can be merged */
    while (new_seg->write_offset < heap.seg_size * SEG_MERGE_STOP_RATIO
            && n_merged < at_most_n_seg) {

        curr_seg = segs_to_merge[n_merged++];
        curr_seg_id = curr_seg->seg_id;

        _seg_copy(new_seg_id, curr_seg_id, &cutoff_freq, SEG_MERGE_TARGET_RATIO);
        accessible = __atomic_exchange_n(&(curr_seg->accessible), 0,
                __ATOMIC_RELAXED);
        ASSERT(accessible == 1);

        _seg_wait_refcnt(curr_seg_id);
        pthread_mutex_lock(&heap.mtx);
        if (n_merged - 1 == 0) {
            _replace_seg_in_chain(new_seg_id, curr_seg_id);
        } else {
            _rm_seg_from_ttl_bucket(curr_seg_id);
        }

        seg_return_seg(curr_seg_id);
        pthread_mutex_unlock(&heap.mtx);
    }

    ASSERT(n_merged > 0);

    /* if no seg has active object */
    if (new_seg->occupied_size <= 8) {
        new_seg->accessible = 0;

        pthread_mutex_lock(&heap.mtx);
        _rm_seg_from_ttl_bucket(new_seg_id);
        seg_return_seg(new_seg_id);
        pthread_mutex_unlock(&heap.mtx);

        log_warn("merged %d segments with no active objects, "
                 "return reserved seg %d", n_merged, new_seg_id);
        for (int i = 0; i < n_merged; i++)
            seg_print(segs_to_merge[i]->seg_id);

        return last_seg_next_seg_id;
    } else {
        /* changed the status of un-merged seg */
        for (int i = n_merged; i < at_most_n_seg; i++) {
            uint8_t evictable = __atomic_exchange_n(
                    &segs_to_merge[i]->evictable, 1, __ATOMIC_RELAXED);
            ASSERT(evictable == 0);
        }

        /* in seg_copy, we could copy over unused bytes */
        memset(seg_get_data_start(new_seg_id) + new_seg->write_offset,
                0, heap.seg_size - new_seg->write_offset);
        new_seg->evictable = 1;

        /* print stat */
        char merged_segs[1024];
        int pos = 0;
        for (int i = 0; i < n_merged; i++) {
            pos += snprintf(merged_segs + pos, 1024-pos, "%d, ", segs_to_merge[i]->seg_id);
        }
        log_info("ttl %d, merged %d/%d segs (%s) to seg %d, "
                 "curr #free segs %d, new seg offset %d, occupied size %d, "
                 "%d items",
                new_seg->ttl, n_merged, at_most_n_seg, merged_segs, new_seg_id,
                heap.n_free_seg, new_seg->write_offset,
                new_seg->occupied_size, new_seg->n_item);
    }

    log_verb("***************************************************");
    INCR_N(seg_metrics, seg_merge, n_merged);

    return heap.segs[new_seg_id].next_seg_id;
}
#endif







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
        heap.n_free_seg = 0;
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
    heap.seg_size = option_uint(&seg_options->seg_size);
    heap.heap_size = option_uint(&seg_options->heap_mem);
    log_verb("cache size %" PRIu64, heap.heap_size);

    heap.free_seg_id = -1;
    heap.prealloc = option_bool(&seg_options->prealloc);
    heap.prefault = option_bool(&seg_options->datapool_prefault);

    heap.poolpath = option_str(&seg_options->datapool_path);
    heap.poolname = option_str(&seg_options->datapool_name);

    heap.n_reserved_seg = 1;
    heap.n_reserved_seg = option_uint(&seg_options->seg_n_thread);

    use_cas = option_bool(&seg_options->use_cas);

    hashtable_setup(option_uint(&seg_options->hash_power));

    if (_seg_heap_setup() != CC_OK) {
        log_crit("Could not setup seg heap info");
        goto error;
    }

    ttl_bucket_setup();

    segevict_setup(option_uint(&options->evict_opt), heap.max_nseg);

    start_background_thread(NULL);

    seg_initialized = true;

    return;

error:
    seg_teardown();
    exit(EX_CONFIG);
}
