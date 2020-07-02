
#include "segevict.h"
#include "seg.h"

/* I think this does not need to be a parameter */
#define UPDATE_INTERVAL 5

static bool segevict_initialized;
static struct seg_evict_info evict;

/* maybe we should use # of req instead of real time to make decision */
static inline bool
_should_rerank(void)
{
    bool rerank;
    proc_time_i curr_sec, prev_sec;
    curr_sec = time_proc_sec();
    prev_sec = __atomic_load_n(&evict.last_update_time, __ATOMIC_RELAXED);
    rerank = prev_sec == -1 || curr_sec - prev_sec > UPDATE_INTERVAL;

    if (rerank) {
        __atomic_store_n(&evict.last_update_time, curr_sec, __ATOMIC_RELAXED);
    }

    return rerank;
}

static inline int
_cmp_seg_FIFO(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    /* avoid segments that are currently being written to */
    if (seg1->w_refcount || seg1->in_free_pool) {
        return 1;
    }
    if (seg2->w_refcount || seg2->in_free_pool) {
        return -1;
    }

    return seg1->create_at - seg2->create_at;
}

static inline int
_cmp_seg_CTE(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (seg1->w_refcount || seg1->in_free_pool) {
        return 1;
    }
    if (seg2->w_refcount || seg2->in_free_pool) {
        return -1;
    }

    return (seg1->create_at + seg1->ttl) - (seg2->create_at + seg2->ttl);
}

static inline int
_cmp_seg_util(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (seg1->w_refcount || seg1->in_free_pool) {
        return 1;
    }
    if (seg2->w_refcount || seg2->in_free_pool) {
        return -1;
    }

    /* need to avoid the segments that are newly allocated */
    //    seg1->next_seg_id == -1

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
    if (!_should_rerank()) {
        return;
    }

    evict.idx_rseg = 0;

    int (*cmp)(const void *, const void *) = NULL;

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

    ASSERT(cmp != NULL);
    qsort(evict.ranked_seg_id, evict.nseg, sizeof(uint32_t), cmp);


    log_debug("ranked seg id %u %u %u %u %u %u %u %u %u ...",
            evict.ranked_seg_id[0], evict.ranked_seg_id[1],
            evict.ranked_seg_id[2], evict.ranked_seg_id[3],
            evict.ranked_seg_id[4], evict.ranked_seg_id[5],
            evict.ranked_seg_id[6], evict.ranked_seg_id[7],
            evict.ranked_seg_id[8]);
    _seg_print(evict.ranked_seg_id[0]);
    _seg_print(evict.ranked_seg_id[1]);
    _seg_print(evict.ranked_seg_id[2]);
    _seg_print(evict.ranked_seg_id[3]);

//    log_debug("ranked seg %d - craete at %" PRId32 ", TTL %" PRId32
//              ", write offset %" PRIu32 ", occupied size %" PRIu32,
//            heap.segs[evict.ranked_seg_id[0]].seg_id,
//            heap.segs[evict.ranked_seg_id[0]].create_at,
//            heap.segs[evict.ranked_seg_id[0]].ttl,
//            heap.segs[evict.ranked_seg_id[0]].write_offset,
//            heap.segs[evict.ranked_seg_id[0]].occupied_size)
}


evict_rstatus_e
least_valuable_seg(uint32_t *seg_id)
{
    ASSERT(heap.nseg == heap.max_nseg);

    if (evict.policy == EVICT_RANDOM) {
        uint32_t i = 0;
        *seg_id = rand() % heap.nseg;
        while (heap.segs[*seg_id].w_refcount > 0 && i <= heap.max_nseg) {
            /* transition to linear search */
            *seg_id = (*seg_id + 1) % heap.max_nseg;
            i++;
        }
        if (i == heap.max_nseg) {
            log_warn("unable to find a segment that has no writer");
            return EVICT_NO_SEALED_SEG;
        } else {
            return EVICT_OK;
        }
    } else {
        _rank_seg();

        int32_t idx = __atomic_fetch_add(&evict.idx_rseg, 1, __ATOMIC_RELAXED);
        *seg_id = evict.ranked_seg_id[idx];
        /* it is OK if we read a staled seg.sealed because we will double check
         * when we perform real eviction */
        while (heap.segs[*seg_id].w_refcount > 0 && idx < evict.nseg) {
            idx = __atomic_fetch_add(&evict.idx_rseg, 1, __ATOMIC_RELAXED);
            *seg_id = evict.ranked_seg_id[idx];
        }
        if (idx >= evict.nseg) {
            log_warn("unable to find a segment that is not sealed");
            __atomic_store_n(&evict.idx_rseg, 0, __ATOMIC_RELAXED);
            /* better return a less utilized one */
            return EVICT_NO_SEALED_SEG;
        }
        //        *seg_id = evict.ranked_seg_id[evict.idx_rseg++];
        ASSERT(heap.segs[*seg_id].in_free_pool == 0);

        return EVICT_OK;
    }
}


void
segevict_teardown(void)
{
    cc_free(evict.ranked_seg_id);

    segevict_initialized = false;
}

void
segevict_setup(evict_policy_e ev_policy, uint32_t nseg)
{
    uint32_t i = 0;

    if (segevict_initialized) {
        log_warn("segevict has already initialized");
        segevict_teardown();
    }

    evict.last_update_time = -1;
    evict.policy = ev_policy;
    evict.nseg = nseg;
    evict.ranked_seg_id = cc_zalloc(sizeof(uint32_t) * nseg);
    evict.idx_rseg = 0;

    for (i = 0; i < nseg; i++) {
        evict.ranked_seg_id[i] = i;
    }

    srand(time(NULL));
    segevict_initialized = true;
}
