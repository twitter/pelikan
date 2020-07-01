
#include "segevict.h"
#include "seg.h"

/* I think this does not need to be a parameter */
#define UPDATE_INTERVAL 5

static bool segevict_initialized;
static struct seg_evict_info evict;

/* maybe we should use # of req instead of real time to make decision */
static inline bool
_should_rerank()
{
    bool rerank;
    proc_time_i curr_sec, prev_sec;
    curr_sec = time_proc_sec();
    prev_sec = __atomic_load_n(&evict.last_update_time, __ATOMIC_RELAXED);
    rerank = prev_sec == -1 || curr_sec - prev_sec > UPDATE_INTERVAL;

    if (rerank){
        __atomic_store_n(&evict.last_update_time, curr_sec, __ATOMIC_RELAXED);
    }

    return rerank;
}

static inline int
_cmp_seg_FIFO(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (!seg1->sealed) {
        return 1;
    }
    if (!seg2->sealed) {
        return -1;
    }

    return seg1->create_at - seg2->create_at;
}

static inline int
_cmp_seg_CTE(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (!seg1->sealed) {
        return 1;
    }
    if (!seg2->sealed) {
        return -1;
    }

    return (seg1->create_at + seg1->ttl) - (seg2->create_at + seg2->ttl);
}

static inline int
_cmp_seg_util(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (!seg1->sealed) {
        return 1;
    }
    if (!seg2->sealed) {
        return -1;
    }

    return seg1->occupied_size - seg2->occupied_size;
}

static inline int
_cmp_seg_smart(const void *d1, const void *d2)
{
    /* we may able to use MINHASH to calculate the number of active items */

    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];
    return seg1->n_hit - seg2->n_hit;
}

// static inline void
//_predict_n_hit(void)
//{
//    ;
//}


static inline void
_rank_seg(void)
{
    printf("try to rank seg %d\n", time_proc_sec());
    if (!_should_rerank()) {
        return;
    }
    printf("rank seg\n");

    evict.idx_rseg_dram = 0;
    evict.idx_rseg_pmem = 0;

    int (*cmp)(const void *, const void *);

    switch (evict.policy) {
    case EVICT_FIFO:
        cmp = _cmp_seg_FIFO;
        break;
    case EVICT_CTE:
        cmp = _cmp_seg_CTE;
        break;
    case EVICT_UTIL:
        cmp = _cmp_seg_util;
        break;
    case EVICT_SMART:
        cmp = _cmp_seg_smart;
        break;
    default:
        NOT_REACHED();
    }

    if (evict.nseg_dram > 0) {
        qsort(evict.ranked_seg_id_dram, evict.nseg_dram, sizeof(uint32_t), cmp);
    }
    if (evict.nseg_pmem > 0) {
        qsort(evict.ranked_seg_id_pmem, evict.nseg_pmem, sizeof(uint32_t), cmp);
    }


    printf("ranked seg id %u %u %u %u\n", evict.ranked_seg_id_dram[0],
            evict.ranked_seg_id_dram[1], evict.ranked_seg_id_dram[2],
            evict.ranked_seg_id_dram[3]);
}


evict_rstatus_e
least_valuable_seg_dram(uint32_t *seg_id)
{
    ASSERT(heap.nseg_dram == heap.max_nseg_dram);

    if (evict.policy == EVICT_RANDOM) {
        uint32_t i = 0;
        *seg_id = rand() % heap.nseg_dram;
        while (heap.segs[*seg_id].sealed == 0) {
            /* transition to linear search */
            *seg_id = (*seg_id + 1) % heap.max_nseg_dram;
        }
        if (i == heap.max_nseg_dram) {
            log_warn("unable to find a segment that is not sealed");
            return EVICT_NO_SEALED_SEG;
        } else {
            return EVICT_OK;
        }
    } else {
        _rank_seg();

        *seg_id = evict.ranked_seg_id_dram[evict.idx_rseg_dram];
        /* it is OK if we read a staled seg.sealed because we will double check
         * when we perform real eviction */
        while (heap.segs[*seg_id].sealed == 0 &&
                evict.idx_rseg_dram < evict.nseg_dram) {
            *seg_id = evict.ranked_seg_id_dram[++evict.idx_rseg_dram];
            ;
        }
        if (evict.idx_rseg_dram >= evict.nseg_dram) {
            log_warn("unable to find a segment that is not sealed");
            evict.idx_rseg_dram = 0;
            /* better return a less utilized one */
            return EVICT_NO_SEALED_SEG;
        }
        *seg_id = evict.ranked_seg_id_dram[evict.idx_rseg_dram++];
        return EVICT_OK;
    }
}

evict_rstatus_e
least_valuable_seg_pmem(uint32_t *seg_id)
{
    ASSERT(heap.nseg_pmem == heap.max_nseg_pmem);
    if (evict.policy == EVICT_RANDOM) {
        return rand() % heap.nseg_pmem;
    } else {
        _rank_seg();
        return evict.ranked_seg_id_pmem[evict.idx_rseg_pmem++];
    }

    return 0;
}

void
segevict_teardown(void)
{
    cc_free(evict.ranked_seg_id_dram);
    cc_free(evict.ranked_seg_id_pmem);

    segevict_initialized = false;
}

void
segevict_setup(evict_policy_e ev_policy, uint32_t nseg_dram, uint32_t nseg_pmem)
{
    uint32_t i = 0;

    if (segevict_initialized) {
        log_warn("segevict has already initialized");
        segevict_teardown();
    }

    evict.last_update_time = -1;
    evict.policy = ev_policy;
    evict.nseg_dram = nseg_dram;
    evict.nseg_pmem = nseg_pmem;
    evict.ranked_seg_id_dram = cc_zalloc(sizeof(uint32_t) * nseg_dram);
    evict.ranked_seg_id_pmem = cc_zalloc(sizeof(uint32_t) * nseg_pmem);
    evict.idx_rseg_dram = 0;
    evict.idx_rseg_pmem = 0;

    for (i = 0; i < nseg_dram; i++) {
        evict.ranked_seg_id_dram[i] = i;
    }

    for (i = 0; i < nseg_pmem; i++) {
        evict.ranked_seg_id_pmem[i] = i;
    }

    srand(time(NULL));
    segevict_initialized = true;
}
