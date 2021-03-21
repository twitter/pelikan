
#include "segevict.h"
#include "seg.h"

/* I think this does not need to be a parameter */
#define UPDATE_INTERVAL 1

static bool segevict_initialized;

struct seg_evict_info evict_info;

extern seg_metrics_st        *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];

char *eviction_policy_names[] = {
    "None",
    "Random",
    "FIFO",
    "CTE",
    "UTIL",
    "MERGE_FIFO",

    "INVALID",
};

/**
 * find the least valuable segment in DRAM
 * return seg_id
 */
evict_rstatus_e
least_valuable_seg(int32_t *seg_id);

bool
seg_evictable(struct seg *seg)
{
    if (seg == NULL) {
        return false;
    }

    bool is_evictable = seg->w_refcount == 0;
    /* although we check evictable here, we will check again after
     * we grab the lock, so this is part of the opportunistic concurrency control */
    is_evictable = is_evictable &&
        (seg->evictable == 1) && (seg->next_seg_id != -1);

    /* a magic number - we don't want to merge just created seg */
    /* TODO(jason): the time needs to be adaptive */
    is_evictable = is_evictable
        && (time_proc_sec() - seg->create_at >= evict_info.seg_mature_time);

//    is_evictable = is_evictable && (time_proc_sec() - seg->create_at) > 2;

    /* don't merge segments that will expire soon */
    is_evictable = is_evictable &&
        seg->create_at + seg->ttl - time_proc_sec() > 20;

    return is_evictable;
}

evict_rstatus_e
seg_evict(int32_t *evicted_seg_id)
{
    evict_rstatus_e status;

    status = least_valuable_seg(evicted_seg_id);
    if (status == EVICT_NO_AVAILABLE_SEG) {
        log_warn("unable to find seg to evict");
        INCR(seg_metrics, seg_evict_ex);

        return EVICT_NO_AVAILABLE_SEG;
    }

    log_verb("evict segment %"PRId32, *evicted_seg_id);

    if (rm_all_item_on_seg(*evicted_seg_id, SEG_EVICTION)) {
        INCR(seg_metrics, seg_evict);

        return EVICT_OK;
    }

    *evicted_seg_id = -1;
    return EVICT_CANNOT_LOCK_SEG;
}

/* maybe we should use # of req instead of real time to make decision */
static inline bool
should_rerank(void)
{
    static bool need_rerank;
    proc_time_i prev_sec;
    prev_sec = evict_info.last_update_time;

    need_rerank =
        prev_sec == -1 || time_proc_sec() - prev_sec > UPDATE_INTERVAL;

    need_rerank = need_rerank || heap.max_nseg - evict_info.idx_rseg < 8;

    return need_rerank;
}

static inline int
cmp_seg_FIFO(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *) d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *) d2];

    /* avoid segments that are currently being written to */
    if (!seg_evictable(seg1)) {
        return 1;
    }
    if (!seg_evictable(seg2)) {
        return -1;
    }

    return MAX(seg1->create_at, seg1->merge_at) -
        MAX(seg2->create_at, seg2->merge_at);
}

static inline int
cmp_seg_CTE(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *) d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *) d2];

    if (!seg_evictable(seg1)) {
        return 1;
    }
    if (!seg_evictable(seg2)) {
        return -1;
    }

    return (seg1->create_at + seg1->ttl) - (seg2->create_at + seg2->ttl);
}

static inline int
cmp_seg_util(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *) d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *) d2];

    if (!seg_evictable(seg1)) {
        return 1;
    }
    if (!seg_evictable(seg2)) {
        return -1;
    }

    return seg1->live_bytes - seg2->live_bytes;
}

static inline void
rank_seg(void)
{
    evict_info.idx_rseg = 0;

    int
    (*cmp)(const void *, const void *) = NULL;

    switch (evict_info.policy) {
        case EVICT_FIFO:cmp = cmp_seg_FIFO;
            break;
        case EVICT_CTE:cmp = cmp_seg_CTE;
            break;
        case EVICT_UTIL:cmp = cmp_seg_util;
            break;
        default:NOT_REACHED();
    }

    ASSERT(cmp != NULL);
    qsort(evict_info.ranked_seg_id, heap.max_nseg, sizeof(uint32_t), cmp);

//    log_debug("ranked seg id %u %u %u %u %u %u %u %u %u %u ...",
//        evict_info.ranked_seg_id[0], evict_info.ranked_seg_id[1],
//        evict_info.ranked_seg_id[2], evict_info.ranked_seg_id[3],
//        evict_info.ranked_seg_id[4], evict_info.ranked_seg_id[5],
//        evict_info.ranked_seg_id[6], evict_info.ranked_seg_id[7],
//        evict_info.ranked_seg_id[8], evict_info.ranked_seg_id[9]);
//    SEG_PRINT(evict_info.ranked_seg_id[0], "", log_debug);
//    SEG_PRINT(evict_info.ranked_seg_id[1], "", log_debug);
//    SEG_PRINT(evict_info.ranked_seg_id[2], "", log_debug);
//    SEG_PRINT(evict_info.ranked_seg_id[3], "", log_debug);

    evict_info.last_update_time = time_proc_sec();
}

evict_rstatus_e
least_valuable_seg(int32_t *seg_id)
{
    struct seg *seg;
    uint32_t   i = 0;

    if (evict_info.policy == EVICT_RANDOM) {
        *seg_id = rand() % heap.max_nseg;
        seg = &heap.segs[*seg_id];
        while ((!seg_evictable(seg)) && i <= heap.max_nseg) {
            /* transition to linear search */
            *seg_id = (*seg_id + 1) % heap.max_nseg;
            seg = &heap.segs[*seg_id];
            i++;
        }
        if (i == heap.max_nseg) {
            return EVICT_NO_AVAILABLE_SEG;
        }
        else {
            return EVICT_OK;
        }
    }
    else {
        pthread_mutex_lock(&evict_info.mtx);

        if (should_rerank()) {
            rank_seg();
        }

        *seg_id = evict_info.ranked_seg_id[evict_info.idx_rseg];
        seg = &heap.segs[*seg_id];
        bool reranked = false;

        while (!seg_evictable(seg) && i < heap.max_nseg) {
            i++;
            if (!reranked && evict_info.idx_rseg + i >= heap.max_nseg) {
                rank_seg();
                i = 0;
                reranked = true;
            }
            *seg_id = evict_info.ranked_seg_id[evict_info.idx_rseg + i];
            seg = &heap.segs[*seg_id];
        }

        if (i >= heap.max_nseg) {
            rank_seg();

            pthread_mutex_unlock(&evict_info.mtx);

            *seg_id = -1;
            return EVICT_NO_AVAILABLE_SEG;
        }
        else {
            evict_info.idx_rseg = (evict_info.idx_rseg + i + 1) % heap.max_nseg;

            pthread_mutex_unlock(&evict_info.mtx);

            return EVICT_OK;
        }

    }
}

void
segevict_teardown(void)
{
    cc_free(evict_info.ranked_seg_id);

    segevict_initialized = false;
}

void
segevict_setup(evict_policy_e ev_policy, uintmax_t seg_mature_time)
{
    uint32_t i = 0;

    if (segevict_initialized) {
        log_warn("segevict has already initialized");

        segevict_teardown();
    }

    evict_info.last_update_time = -1;
    evict_info.policy           = ev_policy;
    evict_info.ranked_seg_id    = cc_zalloc(sizeof(int32_t) * heap.max_nseg);
    evict_info.idx_rseg         = 0;
    pthread_mutex_init(&evict_info.mtx, NULL);

    for (i = 0; i < heap.max_nseg; i++) {
        evict_info.ranked_seg_id[i] = i;
    }

    /* initialize merged-based eviction policy */
    struct merge_opts *mopt = &evict_info.merge_opt;
    mopt->target_ratio = 1.0 / mopt->seg_n_merge;
    /* stop if the bytes on the merged seg is more than the threshold */
    mopt->stop_ratio   = mopt->target_ratio * (mopt->seg_n_merge - 1) + 0.05;
    mopt->stop_bytes   = (int32_t) (heap.seg_size * mopt->stop_ratio);

    srand(time(NULL));
    segevict_initialized = true;
}
