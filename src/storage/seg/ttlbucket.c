
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

    ttl_bucket = &ttl_buckets[ttl_bucket_idx];

    /* a ttl_bucket has either no seg, or at least one seg that is not sealed */
    curr_seg_id = __atomic_load_n(&ttl_bucket->last_seg_id, __ATOMIC_RELAXED);

    if (curr_seg_id != -1) {
        /* optimistic reservation, roll back if failed */
        curr_seg = &heap.segs[curr_seg_id];
        offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_RELAXED);
    }

    while (curr_seg_id == -1 || offset + sz > heap.seg_size) {
        if (curr_seg_id != -1) {
            /* current seg runs out of space, roll back offset */
            __atomic_fetch_sub(&(curr_seg->write_offset), sz, __ATOMIC_RELAXED);
        }

        new_seg_id = seg_get_new();

        if (new_seg_id == -1) {
            log_error("cannot get new segment");
            return NULL;
        }

        new_seg = &heap.segs[new_seg_id];
        new_seg->ttl = ttl_bucket->ttl;
        /* lock is always needed when we update seg list,
         * but we can change this to per-TTL lock if we need to */

        pthread_mutex_lock(&heap.mtx);
        /* pass the lock, need to double check whether the last_seg
         * has changed or not (optimistic alloc) */
        if (curr_seg_id != ttl_bucket->last_seg_id) {
            /* roll back */
            seg_return_seg(new_seg_id);
            new_seg_id = ttl_bucket->last_seg_id;

            INCR(seg_metrics, seg_return);
            log_verb("return segment (id %" PRId32 ") to global pool",
                    new_seg_id);
        } else {
            if (ttl_bucket->first_seg_id == -1) {
                ttl_bucket->first_seg_id = new_seg_id;
                ASSERT(ttl_bucket->last_seg_id == -1);
                ttl_bucket->last_seg_id = new_seg_id;
            } else {
                /* I don't atomic load last_seg_id because
                 * it is only written within critical section */
                heap.segs[curr_seg_id].next_seg_id = new_seg_id;
                new_seg->prev_seg_id = curr_seg_id;
                ttl_bucket->last_seg_id = new_seg_id;
            }
            ttl_bucket->n_seg += 1;
            PERTTL_INCR(ttl_bucket_idx, seg_curr);

            log_verb("link a new segment (id %" PRId32
                     ") to ttl bucket %" PRIu32 ", now %" PRId32 " segments",
                    new_seg_id, ttl_bucket_idx, ttl_bucket->n_seg);
        }

        pthread_mutex_unlock(&heap.mtx);

        curr_seg_id = new_seg_id;
        curr_seg = &heap.segs[curr_seg_id];
        offset = __atomic_fetch_add(
                &(curr_seg->write_offset), sz, __ATOMIC_RELAXED);
    }


    uint32_t occupied_size = __atomic_add_fetch(
            &(curr_seg->occupied_size), sz, __ATOMIC_RELAXED);
    ASSERT(occupied_size <= heap.seg_size);

    __atomic_add_fetch(&curr_seg->n_item, 1, __ATOMIC_RELAXED);

    seg_data = seg_get_data_start(curr_seg_id);
    ASSERT(seg_data != NULL);
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
