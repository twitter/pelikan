
#include "ttlbucket.h"
#include "seg.h"
#include "item.h"

#include <sys/errno.h>
#include <pthread.h>

struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];

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
    struct item       *it;
    struct ttl_bucket *ttl_bucket;
    struct seg        *curr_seg, *new_seg = NULL;

    uint8_t  *seg_data_start = NULL;
    uint32_t offset          = 0;  /* offset of the reserved item in the seg */

    ttl_bucket = &ttl_buckets[ttl_bucket_idx];
    log_verb("ttl_bucket %p", ttl_bucket);

    curr_seg = TAILQ_LAST(&ttl_bucket->seg_q, seg_tqh);
    if (curr_seg != NULL) {
        seg_data_start = seg_get_data_start(curr_seg->seg_id);
        offset         = __atomic_fetch_add(&(curr_seg->write_offset),
            sz, __ATOMIC_RELAXED);
    }

    /* use while instead if: it is possible that a newly allocated seg is
     * quickly filled by other threads in a write-heavy workload */
    while (curr_seg == NULL || offset + sz > heap.seg_size) {
        /* current seg runs out of space */
        if (curr_seg != NULL) {
            __atomic_fetch_sub(&(curr_seg->write_offset), sz, __ATOMIC_RELAXED);
        }

        if (pthread_mutex_lock(&ttl_bucket->mtx) != 0) {
            log_error("fail to lock when allocating a new seg");
            return NULL;
        }
        /* pass the lock, it could be either we need to grab a new seg,
         * or someone has already grabbed a seg, check first */
        new_seg = TAILQ_LAST(&ttl_bucket->seg_q, seg_tqh);
        if (curr_seg == new_seg) {
            /* grab a new seg, link to the end of current seg */
            /* TODO(jason): if scalability becomes a problem
             * we could move seg_get out of lock with the cost of
             * unnecessary eviction
             */
            new_seg = seg_get_new();        /* TODO(jason): set create_at */
            new_seg->ttl = ttl_bucket->ttl;
            TAILQ_INSERT_TAIL(&ttl_bucket->seg_q, new_seg, seg_tqe);
            if (curr_seg) {
                curr_seg->sealed = 1;
            }
            PERTTL_INCR(ttl_bucket_idx, seg_curr);
        }
        pthread_mutex_unlock(&ttl_bucket->mtx);
        curr_seg       = new_seg;
        seg_data_start = seg_get_data_start(curr_seg->seg_id);
        offset         = __atomic_fetch_add(&(curr_seg->write_offset),
            sz, __ATOMIC_RELAXED);
    }

    uint32_t occupied_size = __atomic_add_fetch(&(curr_seg->occupied_size),
        sz, __ATOMIC_RELAXED);
    ASSERT(occupied_size <= heap.seg_size);

    ASSERT(seg_data_start != NULL);
    it = (struct item *) (seg_data_start + offset);
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
            pthread_mutex_init(&ttl_bucket->mtx, NULL);
            TAILQ_INIT(&ttl_bucket->seg_q);
        }
    }
}

void
ttl_bucket_teardown(void)
{
    ;
}

