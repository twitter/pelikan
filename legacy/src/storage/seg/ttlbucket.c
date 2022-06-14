
#include "ttlbucket.h"
#include "item.h"
#include "seg.h"

#include <pthread.h>
#include <sys/errno.h>

extern struct ttl_bucket     ttl_buckets[MAX_N_TTL_BUCKET];
extern seg_metrics_st        *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];
__thread int32_t             local_last_seg[MAX_N_TTL_BUCKET] = {0};


/* reserve the size of an incoming item in the last segment of the TTL bucket,
 * if the segment does not have enough space,
 * grab a new segment and connect to the seg chain
 * seg_id is used to return the id of the segment which the object will be
 * written to
 */
#ifndef USE_THREAD_LOCAL_SEG
/* use thread local seg requires reserving one seg per thread per active TTL
 * bucket, which is expensive when there is no need for high scalability,
 * Segcache can scale to 8 cores without turning this on */
struct item *
ttl_bucket_reserve_item(int32_t ttl_bucket_idx, size_t sz, int32_t *seg_id)
{
    struct item       *it;
    struct ttl_bucket *ttl_bucket = &ttl_buckets[ttl_bucket_idx];
    int32_t           curr_seg_id, new_seg_id;
    struct seg        *curr_seg   = NULL, *new_seg = NULL;

    uint8_t *seg_data  = NULL;
    int32_t offset     = 0; /* offset of the reserved item in the seg */
    uint8_t accessible = true;

    curr_seg_id = ttl_bucket->last_seg_id;

    /* rolling back write_offset is a terrible idea, it causes data corruption
     * in the situation when multiple threads rolling back at the same time
     * 1. one solution is to use per-ttl lock, but given this is on the
     * critical path of insert, I would rather not have a big lock,
     * 2. the other solution is to use cas, but under contended situation,
     * cas is not significantly better than mutex
     * (4 vs 8us on E5 v4 CPU with 64 threads, compared to atomic_add 1000 ns)
     * 3. another solution is roll back only after linking new seg to ttl,
     * but it is not clean enough
     * 4. the solution used here is to not do roll back, since the seg is not
     * changed after writing, we can safely detect end of seg during eviction
     */

    if (curr_seg_id != -1) {
        /* increment offset by sz */
        curr_seg   = &heap.segs[curr_seg_id];
        accessible = seg_is_accessible(curr_seg_id);
        if (accessible) {
            offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
        }
    }

    while (curr_seg_id == -1 || offset + sz > heap.seg_size || (!accessible)) {
        /* we need to get a new segment */
        if (offset + sz > heap.seg_size && offset < heap.seg_size) {
            /* we cannot roll back offset due to data race,
             * but we need to explicitly clear rest of the segment
             * so that we know it is the end of segment, this is because
             * we do not zero the segment during initialization */
            seg_data = get_seg_data_start(curr_seg_id);
            memset(seg_data + offset, 0, heap.seg_size - offset);
        }

        new_seg_id = seg_get_new();

        if (new_seg_id == -1) {
            dump_seg_info();
#if defined CC_ASSERT_PANIC || defined(CC_ASSERT_LOG)
            ASSERT(0);
#endif
            log_warn("cannot get new segment");
            return NULL;
        }
        new_seg = &heap.segs[new_seg_id];
        new_seg->ttl = ttl_bucket->ttl;

        /* TODO(juncheng): update to TTL lock */
        if (pthread_mutex_lock(&heap.mtx) != 0) {
            log_error("unable to lock mutex");
            return NULL;
        }
        /* pass the lock, need to double check whether the last_seg
         * has changed (optimistic alloc) this can change either because
         * another thread has linked a new segment or
         * curr_seg is expired and remove */
        /* TODO(jason): we can add to the head instead of tail */
        if (curr_seg_id != ttl_bucket->last_seg_id &&
            ttl_bucket->last_seg_id != -1) {
            /* roll back */
            INCR(seg_metrics, seg_return);

            seg_add_to_freepool(new_seg_id, SEG_CONCURRENT_GET);
            new_seg_id = ttl_bucket->last_seg_id;

        }
        else {
            /* last seg has not changed */
            if (ttl_bucket->first_seg_id == -1) {
                /* the first seg of the bucket */
                ASSERT(ttl_bucket->last_seg_id == -1);

                ttl_bucket->first_seg_id = new_seg_id;
            }
            else {
                ASSERT(curr_seg != NULL);
                ASSERT(ttl_bucket->last_seg_id != -1);

                heap.segs[curr_seg_id].next_seg_id = new_seg_id;
            }

            /* it prev seg has a short TTL and has expired,
             * ttl_bucket->last_seg_id would be -1,
             * different from curr_seg_id */
            new_seg->prev_seg_id    = ttl_bucket->last_seg_id;
            ttl_bucket->last_seg_id = new_seg_id;
            ASSERT(new_seg->next_seg_id == -1);

            ttl_bucket->n_seg++;

            /* Q(juncheng): can we make it evictable when the seg finishes? */
            bool evictable = __atomic_exchange_n(
                &new_seg->evictable, 1, __ATOMIC_RELAXED);
            ASSERT(evictable == 0);

            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            ASSERT(new_seg->prev_seg_id == curr_seg_id ||
                            new_seg->prev_seg_id == -1);
            log_debug("link seg %6d (offset %d live_bytes %d) to "
                      "ttl bucket %d, ttl %8d, total %d segments, "
                      "prev seg %d (offset %d), first seg %d, last seg %d",
                new_seg_id, new_seg->write_offset, new_seg->live_bytes,
                ttl_bucket_idx,ttl_bucket->ttl,
                ttl_bucket->n_seg, new_seg->prev_seg_id,
                curr_seg_id == -1 ? -1 : __atomic_load_n(
                    &heap.segs[curr_seg_id].write_offset, __ATOMIC_SEQ_CST),
                ttl_bucket->first_seg_id, ttl_bucket->last_seg_id);
        }

        pthread_mutex_unlock(&heap.mtx);

        curr_seg_id = new_seg_id;
        curr_seg    = &heap.segs[curr_seg_id];
        offset      = __atomic_fetch_add(
            &(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
        accessible  = seg_is_accessible(curr_seg_id);
    }

    seg_data = get_seg_data_start(curr_seg_id);
    ASSERT(seg_data != NULL);

    it = (struct item *) (seg_data + offset);
    *seg_id = curr_seg->seg_id;

    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);

    return it;
}
#else
struct item *
ttl_bucket_reserve_item(int32_t ttl_bucket_idx, size_t sz, int32_t *seg_id)
{
    struct item       *it;
    struct ttl_bucket *ttl_bucket = &ttl_buckets[ttl_bucket_idx];
    int32_t           curr_seg_id;
    struct seg        *curr_seg   = NULL;

    uint8_t *seg_data  = NULL;
    int32_t offset     = 0; /* offset of the reserved item in the seg */
    uint8_t accessible = false;

    curr_seg_id = local_last_seg[ttl_bucket_idx] - 1;

    if (curr_seg_id != -1) {
        curr_seg   = &heap.segs[curr_seg_id];
        accessible = seg_is_accessible(curr_seg_id);
        if (accessible) {
            offset = curr_seg->write_offset;
        }
    }

    if (curr_seg_id == -1 || offset + sz > heap.seg_size || (!accessible)) {
        if (offset + sz > heap.seg_size) {
            ASSERT(offset <= heap.seg_size);
            seg_data = get_seg_data_start(curr_seg_id);
            memset(seg_data + offset, 0, heap.seg_size - offset);
        }

        if (curr_seg_id != -1) {
            /* curr seg is not linked to segment chain at this time,
             * link it now */
            if (pthread_mutex_lock(&heap.mtx) != 0) {
                log_error("unable to lock mutex");
                return NULL;
            }

            /* last seg id could be -1 */
            if (ttl_bucket->first_seg_id == -1) {
                ASSERT(ttl_bucket->last_seg_id == -1);

                ttl_bucket->first_seg_id = curr_seg_id;
            }
            else {
                heap.segs[ttl_bucket->last_seg_id].next_seg_id = curr_seg_id;
            }

            curr_seg->prev_seg_id   = ttl_bucket->last_seg_id;
            ttl_bucket->last_seg_id = curr_seg_id;
            ASSERT(curr_seg->next_seg_id == -1);

            ttl_bucket->n_seg += 1;

            bool evictable = __atomic_exchange_n(
                &curr_seg->evictable, 1, __ATOMIC_RELAXED);
            ASSERT(evictable == 0);

            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            log_debug("link seg %d (offset %d occupied_size %d) to "
                      "ttl bucket %d, total %d segments, "
                      "prev seg %d, first seg %d, last seg %d",
                curr_seg_id, curr_seg->write_offset, curr_seg->live_bytes,
                ttl_bucket_idx, ttl_bucket->n_seg, curr_seg->prev_seg_id,
                ttl_bucket->first_seg_id, ttl_bucket->last_seg_id);

            pthread_mutex_unlock(&heap.mtx);
        }

        curr_seg_id = seg_get_new();
        if (curr_seg_id == -1) {
#if defined CC_ASSERT_PANIC || defined(CC_ASSERT_LOG)
            ASSERT(0);
#endif
            log_warn("cannot get new segment");
            return NULL;
        }

        local_last_seg[ttl_bucket_idx] = curr_seg_id + 1;
        curr_seg = &heap.segs[curr_seg_id];
        curr_seg->ttl         = ttl_bucket->ttl;
        curr_seg->next_seg_id = -1;
        offset = curr_seg->write_offset;
    }

    curr_seg->write_offset += sz;
    seg_data = get_seg_data_start(curr_seg_id);
    it       = (struct item *) (seg_data + offset);
    *seg_id = curr_seg->seg_id;

    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);

    return it;
}
#endif

void
ttl_bucket_setup(void)
{
    struct ttl_bucket *ttl_bucket;

    delta_time_i ttl_bucket_intvls[] = {TTL_BUCKET_INTVL1, TTL_BUCKET_INTVL2,
                                        TTL_BUCKET_INTVL3, TTL_BUCKET_INTVL4};

    for (uint32_t i = 0; i < 4; i++) {
        for (uint32_t j = 0; j < N_BUCKET_PER_STEP; j++) {
            ttl_bucket = &(ttl_buckets[i * N_BUCKET_PER_STEP + j]);
            memset(ttl_bucket, 0, sizeof(*ttl_bucket));
            ttl_bucket->ttl               = ttl_bucket_intvls[i] * j + 1;
            ttl_bucket->last_seg_id       = -1;
            ttl_bucket->first_seg_id      = -1;
            ttl_bucket->next_seg_to_merge = -1;
            ttl_bucket->last_cutoff_freq  = 0;
            pthread_mutex_init(&(ttl_bucket->mtx), NULL);
        }
    }
}

void
ttl_bucket_teardown(void)
{
    ;
}
