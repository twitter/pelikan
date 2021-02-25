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
    EVICT_SMART,

    EVICT_INVALID
} evict_policy_e;

typedef enum evict_rstatus {
    EVICT_OK,
    EVICT_NO_SEALED_SEG,

    EVICT_OTHER,
} evict_rstatus_e;


struct seg_evict_info {
    evict_policy_e      policy;
    proc_time_i         last_update_time;
    int32_t             nseg;
    int32_t             *ranked_seg_id;  /* ranked seg ids from the least
                                          * valuable to the most valuable */
    int32_t             idx_rseg;        /* curr index in ranked seg id array */
    pthread_mutex_t     mtx;
};


/**
 * find the least valuable segment in DRAM
 * return seg_id
 */
evict_rstatus_e
least_valuable_seg(int32_t *seg_id);


/* this must be called after seg_setup has finished */
void segevict_setup(evict_policy_e ev_policy, int32_t nseg);

void segevict_teardown(void);

