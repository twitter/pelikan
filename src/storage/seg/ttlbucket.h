#pragma once

#include "constant.h"
#include "seg.h"

#include <cc_queue.h>
#include <limits.h>
#include <stdint.h>
#include <pthread.h>

#define likely(x)      __builtin_expect(!!(x), 1)
#define unlikely(x)    __builtin_expect(!!(x), 0)


/*
 * segment based linked list memory allocator prioritizes TTL expiration over
 * eviction when cache space needs to be reclaimed.
 *
 * We use an array of TTL buckets, each bucket stores the head of a segment list,
 * when memory needs to be reclaimed,
 * TODO(jason): need to think how to implement smart eviction
 *
 * slabclass[]:
 *
 *
 *
 *
 *
 *                               +------------------+             +------------------+
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               |     seg data     |             |     seg data     |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 * +----------------+            |                  |             |                  |
 * |                |            |                  |             |                  |
 * |  TTL bucket 1  |            +--------^---------+             +--------^---------+
 * |                |                     |                                |
 * |                |                     |                                |
 * +----------------+          +----------+----------+          +----------+----------+
 * |                |          |                     |          |                     |
 * |  TTL bucket 2  +---------->     struct seg      +---------->     struct seg      +--------->
 * |                |          |                     |          |                     |
 * |                |          +---------------------+          +---------------------+
 * +----------------+
 * |                |          +---------------------+          +---------------------+
 * |  TTL bucket 3  |          |                     |          |                     |
 * |                +---------->     struct seg      +---------->     struct seg      +--------->
 * |                |          |                     |          |                     |
 * +----------------+          +----------+----------+          +----------+----------+
 * |                |                     |                                |
 * |  TTL bucket 4  |                     |                                |
 * |                |                     |                                |
 * |                |            +--------v---------+             +--------v---------+
 * +----------------+            |                  |             |                  |
 *                               |                  |             |                  |
 *                               |    seg data      |             |    seg data      |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               |                  |             |                  |
 *                               +------------------+             +------------------+
 *
 */


/*
 * because it is inefficient to have a list per ttl, so we bucket ttl into ttl
 * buckets to reduce the number of ttl lists, specifically, we have the following
 * TTL buckets (defined in constant.h)
 *     1s   -     2047s (34m) :  256 buckets each 8s (except last bucket)
 *   2048s  -    32767s (9.1h):  256 buckets each 128s (first 16 bucket not used)
 *  32768s  -   524287s (6.1d):  256 buckets each 2048s (first 16 bucket not used)
 * 524288s  -  8388607s (97d) :  256 buckets each 32768s (first 16 bucket not used)
 *
 * a ttl 45 falls into bucket 5 (45 >> 3 = 5), a ttl 30000 falls into bucket 14,
 * in total 1024 buckets are allocated at start up, and
 * (16+40+8) B * 1024 = 32 KiB DRAM will be consumed
 *
 * NOTE: there are 48 buckets wasted,  it can be used if needed,
 * but the the code complexity will skyrocket
 */


//TAILQ_HEAD(seg_id_tqh, uint32_t);
//TAILQ_ENTRY(uint32_t) seg_id_tqe;

/* we can use a timing wheel here or in seg */
struct ttl_bucket {
    int32_t first_seg_id;
    int32_t last_seg_id;

    pthread_mutex_t mtx;

    delta_time_i    ttl;           /* the min ttl of this bucket */
    uint32_t n_seg;
};

extern struct ttl_bucket ttl_buckets[MAX_TTL_BUCKET];

/**
 * give a TTL, find the index of TTL bucket in ttl_array
 */
static inline uint32_t
find_ttl_bucket_idx(delta_time_i ttl)
{
    uint32_t bucket_idx    = 0;

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

/* return the (min) TTL of the bucket */
static inline delta_time_i
bucket_idx_to_ttl(uint16_t bucket_idx){
    return ttl_buckets[bucket_idx].ttl;
}

void
ttl_bucket_setup(void);

void
ttl_bucket_teardown(void);

/*
 * Reserve an item from the active segment of the ttl bucket.
 * If the active seg of current ttl bucket does not have enough space,
 * we first try to get an empty segment,
 * if there is no free seg,
 *      we check whether there are expired segment,
 *      if yes, remove the seg (refcount must be 0),
 *      if no, we evict one seg,
 * then we link the seg into ttl_bucket, make it the current active seg.
 *
 */
struct item *
ttl_bucket_reserve_item(int32_t ttl_bucket_idx, size_t sz, int32_t *seg_id);

