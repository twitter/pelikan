
#include "ttlbucket.h"
#include "item.h"
#include "seg.h"

#include <pthread.h>
#include <sys/errno.h>

struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];

extern seg_metrics_st *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];


/* I originally wrote using atomics for scalability and ttl_bucket struct
 * size reduction, but it is way too complex and might be incorrect,
 * so fall back to use lock for now.
 * I don't think this will be a scalability bottleneck and
 * the extra space used by lock (48B) and cond var (40B)
 * should not be a problem.
 */
/* reserve the size of an incoming item in the segment,
 * if the segment size is not large enough,
 * we grab a new one and connect to the seg list
 * seg_p and ttl_bucket_idx_p are used to return the seg and ttl_bucket_idx
 */
struct item *
ttl_bucket_reserve_item(uint32_t ttl_bucket_idx, size_t sz)
{
    struct item *it;
    struct ttl_bucket *ttl_bucket;
    int32_t curr_seg_id, new_seg_id;
    struct seg *curr_seg = NULL, *new_seg = NULL;

    uint8_t *seg_data = NULL;
    uint32_t offset = 0; /* offset of the reserved item in the seg */
    uint8_t locked = false;

    ttl_bucket = &ttl_buckets[ttl_bucket_idx];

    /* a ttl_bucket has either no seg, or at least one seg that is not sealed */
    curr_seg_id = ttl_bucket->last_seg_id;

    if (curr_seg_id != -1) {
        /* optimistic reservation, roll back if failed */
        curr_seg = &heap.segs[curr_seg_id];
        offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
        locked = seg_is_locked(curr_seg);
    }

//    log_debug("ttl_bucket_reserve_item: ttl_bucket %u cur seg %d offset %u locked %d",
//            ttl_bucket_idx, curr_seg_id, offset, locked);

    while (curr_seg_id == -1 || offset + sz > heap.seg_size || locked) {
//        log_debug("ttl_bucket_reserve_item: need new seg ttl_bucket %d cur seg %d %d (%d+%d) locked %d", ttl_bucket_idx,
//                curr_seg_id, offset + sz > heap.seg_size, offset, sz, locked);
        if (curr_seg_id != -1) {
            /* current seg runs out of space, roll back offset
             *
             * optimistic design:
             * notice that not using lock around add and sub can cause false
             * full problem when it is highly contended, for example,
             * current offset 600K, thread A adds 500K, then offset too large,
             * before A rolls back the offset change, if thread B, C, D ask for
             * 100K, which should fit in the space, but it will not because
             * add and sub is not in the critical section
             * moving the lock up can solve this problem,
             * but in such highly-contended case, I believe moving the lock
             * will have a large impact on scalability */
            __atomic_fetch_sub(&(curr_seg->write_offset), sz, __ATOMIC_SEQ_CST);
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
         * has changed or not (optimistic alloc) */
        if (curr_seg_id != ttl_bucket->last_seg_id) {
            /* roll back */
            INCR(seg_metrics, seg_return);

            seg_return_seg(new_seg_id);
            new_seg_id = ttl_bucket->last_seg_id;

        } else {
            if (ttl_bucket->first_seg_id == -1) {
                ASSERT(ttl_bucket->last_seg_id == -1);

                ttl_bucket->first_seg_id = new_seg_id;
            } else {
                /* I don't atomic load last_seg_id because
                 * it is only written within critical section */
                heap.segs[curr_seg_id].next_seg_id = new_seg_id;
            }
            new_seg->prev_seg_id = curr_seg_id;
            ttl_bucket->last_seg_id = new_seg_id;
            ASSERT(new_seg->next_seg_id == -1);

            locked = __atomic_exchange_n(&new_seg->locked, 0, __ATOMIC_SEQ_CST);
            ASSERT(locked == 1);

            ttl_bucket->n_seg += 1;

            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            log_verb("link seg %d to ttl bucket %u, now %u segments, prev seg "
                     "%d (offset %d)",
                    new_seg_id, ttl_bucket_idx, ttl_bucket->n_seg, curr_seg_id,
                    __atomic_load_n(&heap.segs[curr_seg_id].write_offset, __ATOMIC_SEQ_CST));
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
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *)(seg_data) == SEG_MAGIC);
#endif

    it = (struct item *)(seg_data + offset);
    it->seg_id = curr_seg->seg_id;

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
            //            pthread_mutex_init(&ttl_bucket->mtx, NULL);
            //            TAILQ_INIT(&ttl_bucket->seg_q);
        }
    }
}

void
ttl_bucket_teardown(void)
{
    ;
}
