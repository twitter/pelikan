#pragma once

#include "constant.h"
#include "seg.h"

#include <cc_queue.h>
#include <limits.h>
#include <stdint.h>
#include <pthread.h>

#define likely(x)      __builtin_expect(!!(x), 1)
#define unlikely(x)    __builtin_expect(!!(x), 0)


/**
 * TTL indexed segment linked list, each segment (after allocation)
 * is linked in one list, either a TTL bucket, or free pool
 *
 * This allows us to perform active TTL expiration - remove all expired
 * objects without search the heap.
 *
 * ** TTL buckets **
 * We use an array of TTL buckets, each bucket stores the head and the tail of
 * the segment list, insert only happens at the tail segment, when tail segment
 * is full, we allocate a new seg and linked it into the list
 *
 * because it is inefficient to have a list per ttl, so we bucket ttl into
 * TTL buckets to reduce the number of ttl lists,
 * specifically, we have the following TTL buckets (defined in constant.h)
 *     1s  -    2047s (34m) : 256 buckets each 8s (except last bucket)
 *   2048s -   32767s (9.1h): 256 buckets each 128s (first 16 bucket not used)
 *  32768s -  524287s (6.1d): 256 buckets each 2048s (first 16 bucket not used)
 * 524288s - 8388607s (97d) : 256 buckets each 32768s (first 16 bucket not used)
 *
 * Example:
 * ttl 45 falls into bucket 5 (45 >> 3 = 5),
 * ttl 30000 falls into bucket 14
 * in total 1024 buckets are allocated at start up, and
 * (16+40+8) B * 1024 = 32 KiB DRAM will be consumed
 *
 * currently there are 48 buckets wasted,  it can be used if needed
 *
 *
 * ** segment headers **
 * we use an array of segment headers, the size of array is the same as the
 * number of segments, each header corresponds to one fixed segment,
 * taking the segment header out of segment has two benefits:
 * 1. because seg header is small, it can be cached in L3 more easily (not confirmed)
 * 2. we update metadata of each segment frequently, separating seg headers
 *      allow us to use slower device for segments (PMem, SSD) with
 *      better performance and low write amplification
 *
 *
 *                                       segment header array
 *                                    ┌────────────────────────┐
 *                                    │                        │
 *                                ┌──▶│    segment header 1    ├──next┐
 *                                │   │                        │      │
 *                                │   ├────────────────────────┤      │
 *                                │   │                        │      │
 *                                │   │    segment header 2    ┣ ━ ━ ━│━ ━
 *  TTL bucket array              │   │                        │      │   ┃
 * ┌────────────────┐  first seg  │   ├────────────────────────┤      │
 * │                ├─────────────┘   │                        │◀─────┘   ┃
 * │  TTL bucket 1  │                 │    segment header 3    │──next┐
 * │                ├─────────────┐   │                        │      │   ┃
 * ├────────────────┤  last seg   │   ├────────────────────────┤      │
 * │                │             └──▶│                        │      │   ┃
 * │  TTL bucket 2  │                 │          ...           │◀─────┘
 * │                │                 │                        │          ┃
 * ├────────────────┤                 ├────────────────────────┤
 * │                │                 │                        │          ┃
 * │      ...       │             ┌──▶│          ...           │
 * │                │             │   │                        │          ┃
 * ├────────────────┤             │   ├────────────────────────┤
 * │                │             │   │                        │◀ ━ ━ ━ ━ ┛
 * │      ...       │             │   │          ...           │━ ━ ━ ━ ━ ┓
 * │                │             │   │                        │
 * ├────────────────┤  first seg  │   ├────────────────────────┤          ┃
 * │                ├─────────────┘   │                        │
 * │ TTL bucket 1022│                 │          ...           │          ┃
 * │                ├─────────────┐   │                        │
 * ├────────────────┤  last seg   │   ├────────────────────────┤          ┃
 * │                │             │   │                        │
 * │ TTL bucket 1023│             │   │          ...           │◀ ━ ━ ━ ━ ┛
 * │                │             │   │                        │
 * └────────────────┘             │   ├────────────────────────┤
 *                                │   │                        │
 *                                └──▶│   segment header N-2   │
 *                                    │                        │
 *                                    ├────────────────────────┤
 *                                    │                        │
 *                                    │   segment header N-1   │
 *                                    │                        │
 *                                    ├────────────────────────┤
 *                                    │                        │
 *                                    │    segment header N    │
 *                                    │                        │
 *                                    └────────────────────────┘
 *
 *
 */


struct ttl_bucket {
    int32_t             first_seg_id;
    int32_t             last_seg_id;
    delta_time_i        ttl;           /* the min ttl of this bucket */
    uint32_t            n_seg;
    int32_t             next_seg_to_merge;
    delta_time_i        last_cutoff_freq;
    pthread_mutex_t     mtx;
};


/**
 * give a TTL, find the index of TTL bucket in ttl_array
 */
static inline uint32_t
find_ttl_bucket_idx(delta_time_i ttl)
{
    uint32_t bucket_idx;

    if (unlikely(ttl == 0)) {
        bucket_idx = MAX_TTL_BUCKET_IDX;
    }
    else if (unlikely(((ttl & ~(TTL_BOUNDARY1 - 1)) == 0))) {
        /* 0 < ttl < TTL_BOUNDARY1 */
        bucket_idx = ttl >> TTL_BUCKET_INTVL_N_BIT1;
    }
    else if ((ttl & ~(TTL_BOUNDARY2 - 1)) == 0) {
        /* TTL_BOUNDARY1 <= ttl < TTL_BOUNDARY2 */
        bucket_idx = (ttl >> TTL_BUCKET_INTVL_N_BIT2) + N_BUCKET_PER_STEP;
    }
    else if ((ttl & ~(TTL_BOUNDARY3 - 1)) == 0) {
        /* TTL_BOUNDARY2 <= ttl < TTL_BOUNDARY3 */
        bucket_idx = (ttl >> TTL_BUCKET_INTVL_N_BIT3) + N_BUCKET_PER_STEP * 2;
    }
    else {
        /* ttl >= TTL_BOUNDARY3 */
        bucket_idx = (ttl >> TTL_BUCKET_INTVL_N_BIT4) + N_BUCKET_PER_STEP * 3;
        if (bucket_idx > MAX_TTL_BUCKET_IDX){
            bucket_idx = MAX_TTL_BUCKET_IDX;
        }
    }

    return bucket_idx;
}


void
ttl_bucket_setup(void);

void
ttl_bucket_teardown(void);

/**
 * Reserve an item from the active segment of the ttl bucket.
 * If the active seg of current ttl bucket does not have enough space,
 * we will get an empty segment
 * then link the seg into ttl_bucket, make it the current active seg.
 *
 */
struct item *
ttl_bucket_reserve_item(int32_t ttl_bucket_idx, size_t sz, int32_t *seg_id);
