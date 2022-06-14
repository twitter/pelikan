#pragma once

#include "constant.h"

#include <time/time.h>
#include <cc_mm.h>
#include <pthread.h>

typedef enum {
    EVICT_NONE = 0,
    EVICT_RANDOM,
    EVICT_FIFO,
    EVICT_CTE,     /* Closet To Expiration */
    EVICT_UTIL,
    EVICT_MERGE_FIFO,

    EVICT_INVALID
} evict_policy_e;

typedef enum evict_rstatus {
    EVICT_OK,
    EVICT_CANNOT_LOCK_SEG,
    EVICT_NO_AVAILABLE_SEG,

    EVICT_OTHER,
} evict_rstatus_e;


struct merge_opts {

    int32_t seg_n_merge;
    int32_t seg_n_max_merge;

    double  target_ratio;
    /* if the bytes on the merged seg is more than the threshold,
     * we stop merge process */
    double  stop_ratio;
    int32_t stop_bytes;

};

struct seg_evict_info {
    evict_policy_e      policy;

    struct merge_opts    merge_opt;

    /* segment younger than seg_mature_time should not be selected */
    int32_t             seg_mature_time;

    proc_time_i         last_update_time;

    int32_t             *ranked_seg_id;  /* ranked seg ids from the least
                                          * valuable to the most valuable */
    int32_t             idx_rseg;        /* curr index in ranked seg id array */

    pthread_mutex_t     mtx;
};


struct seg;
/**
 * check whether a segment can be evicted,
 * a segment cannot be evicted if
 * 1. it is expired or expiring soon
 * 2. is is being evicted by another thread
 * 3. it is the last segment of the chain (active be written to)
 * 4. it is too young (age smaller than seg_mature_time)
 */
bool
seg_evictable(struct seg *seg);

/* evict one segment, return the id of the evicted segment in evicted_seg_id,
 * this function can fail if it cannot find an evictable segment */
evict_rstatus_e
seg_evict(int32_t *evicted_seg_id);

evict_rstatus_e
seg_merge_evict(int32_t *seg_id_ret);

void
segevict_setup(evict_policy_e ev_policy, uintmax_t seg_mature_time);

void
segevict_teardown(void);

