#pragma once

#include "constant.h"
//#include "seg.h"

#include <time/time.h>
#include <cc_mm.h>
#include <pthread.h>

typedef enum {
    EVICT_NONE = 0,
    EVICT_RANDOM,
    EVICT_FIFO,
    EVICT_CTE,     /* Closet To Expiration */
    EVICT_UTIL,
    EVICT_SMART,

    EVICT_INVALID
} evict_policy_e;

typedef enum evict_rstatus {
    EVICT_OK,
    EVICT_NO_SEALED_SEG,

    EVICT_OTHER,
} evict_rstatus_e;


struct seg_evict_info {
    evict_policy_e policy;
    proc_time_i last_update_time;
    uint32_t nseg;
    uint32_t *ranked_seg_id;  /* the least valuable to the most valuable */
    uint32_t idx_rseg;     /* curr index in ranked seg id array */
    pthread_mutex_t mtx;
};


/**
 * find the least valuable segment in DRAM
 * return seg_id
 */
evict_rstatus_e
least_valuable_seg(uint32_t *seg_id);


/* this must be setup update seg_setup has finished */
void segevict_setup(evict_policy_e ev_policy, uint32_t nseg);

void segevict_teardown(void);
