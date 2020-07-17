
#include "ttlbucket.h"
#include "item.h"
#include "seg.h"

#include <pthread.h>
#include <sys/errno.h>

extern struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];
extern seg_metrics_st *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];


/* reserve the size of an incoming item in the segment,
 * if the segment size is not large enough,
 * grab a new one and connect to the seg list
 * seg_id is used to return the seg id
 */
struct item *
ttl_bucket_reserve_item(int32_t ttl_bucket_idx, size_t sz, int32_t *seg_id)
{
    struct item *it;
    struct ttl_bucket *ttl_bucket = &ttl_buckets[ttl_bucket_idx];
    int32_t curr_seg_id, new_seg_id;
    struct seg *curr_seg = NULL, *new_seg = NULL;

    uint8_t *seg_data = NULL;
    int32_t offset = 0; /* offset of the reserved item in the seg */
    uint8_t locked = false;


    curr_seg_id = ttl_bucket->last_seg_id;

    /* rolling back write_offset is a terrible idea, it causes data corruption
     * in the situation when multiple threads rolling back at the same time
     * 1. one solution is to use per-ttl lock, but given this is on the
     * critical path of insert, I would rather not have a big lock,
     * 2. the other solution is to use cas, but under contended situation,
     * cas is not significantly better than mutex
     * (4000 vs 8000 ns on E5 v4 CPU with 64 threads, atomic_add 1000 ns)
     * 3. another solution is roll back only after linking new seg to ttl,
     * but it is not clean enough
     * 4. the solution used here is to not do roll back, since the seg is not
     * changed after writing, we can safely detect end of seg during eviction
     */

    if (curr_seg_id != -1) {
        curr_seg = &heap.segs[curr_seg_id];
        offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
        locked = seg_is_locked(curr_seg);
    }


    while (curr_seg_id == -1 || offset + sz > heap.seg_size || locked) {
        if (curr_seg_id != -1) {
            /* current seg runs out of space, roll back offset
             *
             * optimistic concurrency control:
             * notice that not using lock around add and sub can cause false
             * full problem when it is highly contended, for example,
             * current offset 600K, thread A adds 500K, then offset too large,
             * before A rolls back the offset change, if thread B, C, D ask for
             * 100K, which should fit in the space, but it will not because
             * add and sub is not in the critical section
             * moving the lock up can solve this problem,
             * but in such highly-contended case, I believe moving the lock
             * will have a large impact on scalability */

            //            __atomic_fetch_sub(&(curr_seg->write_offset), sz,
            //            __ATOMIC_SEQ_CST);
        }

        new_seg_id = seg_get_new();

        if (new_seg_id == -1) {
            log_error("cannot get new segment");
            return NULL;
        }
        new_seg = &heap.segs[new_seg_id];
        new_seg->ttl = ttl_bucket->ttl;

        /* TODO(jason): we can check the offset again to reduce the chance
         * of false full problem */

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

            seg_return_seg(new_seg_id);
            new_seg_id = ttl_bucket->last_seg_id;

        } else {
            if (ttl_bucket->first_seg_id == -1) {
                ASSERT(ttl_bucket->last_seg_id == -1);

                ttl_bucket->first_seg_id = new_seg_id;
            } else {
                heap.segs[curr_seg_id].next_seg_id = new_seg_id;
            }
            new_seg->prev_seg_id = curr_seg_id;
            ttl_bucket->last_seg_id = new_seg_id;
            ASSERT(new_seg->next_seg_id == -1);

            locked = __atomic_exchange_n(&new_seg->locked, 0, __ATOMIC_SEQ_CST);
            ASSERT(locked == 1);

            ttl_bucket->n_seg += 1;

            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            log_verb("link seg %" PRIu32 " to ttl bucket %" PRIu32
                     ", total %" PRIu32 " segments, prev seg "
                     "%" PRIu32 " (offset %" PRIu32 ")",
                    new_seg_id, ttl_bucket_idx, ttl_bucket->n_seg, curr_seg_id,
                    curr_seg_id == -1 ?
                            -1 :
                            __atomic_load_n(
                                    &heap.segs[curr_seg_id].write_offset,
                                    __ATOMIC_SEQ_CST));
        }

        pthread_mutex_unlock(&heap.mtx);

        curr_seg_id = new_seg_id;
        curr_seg = &heap.segs[curr_seg_id];
        offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
        locked = seg_is_locked(curr_seg);
    }


    seg_data = seg_get_data_start(curr_seg_id);
    ASSERT(seg_data != NULL);

    it = (struct item *)(seg_data + offset);
    *seg_id = curr_seg->seg_id;

    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);

    return it;
}

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
            ttl_bucket->ttl = ttl_bucket_intvls[i] * j + 1;
            ttl_bucket->last_seg_id = -1;
            ttl_bucket->first_seg_id = -1;
        }
    }
}

void
ttl_bucket_teardown(void)
{
    ;
}
