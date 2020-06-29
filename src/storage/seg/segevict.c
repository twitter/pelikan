
#include "segevict.h"

static bool                  segevict_initialized;
static struct seg_evict_info evict_info;

static inline void
_rank_seg_FIFO(void)
{
    ;
}

static inline void
_rank_seg_TTL(void)
{
    ;
}

static inline void
_rank_seg_utilization(void)
{
    ;
}

static inline void
_rank_seg_smart(void)
{
    ;
}

uint32_t
least_valuable_seg_dram(void)
{
    return 0;
}

uint32_t
least_valuable_seg_pmem(void)
{
    return 0;
}

void
segevict_teardown()
{
    cc_free(evict_info.ranked_seg_id_dram);
    cc_free(evict_info.ranked_seg_id_pmem);
}

void
segevict_setup(evict_policy_e ev_policy, uint32_t nseg_dram, uint32_t nseg_pmem)
{
    evict_info.last_update_time   = 0;
    evict_info.policy             = ev_policy;
    evict_info.max_nseg_dram      = nseg_dram;
    evict_info.max_nseg_pmem      = nseg_pmem;
    evict_info.ranked_seg_id_dram = cc_zalloc(sizeof(uint32_t) * nseg_dram);
    evict_info.ranked_seg_id_pmem = cc_zalloc(sizeof(uint32_t) * nseg_pmem);
}
