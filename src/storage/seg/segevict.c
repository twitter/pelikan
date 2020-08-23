
#include "segevict.h"
#include "seg.h"

/* I think this does not need to be a parameter */
#define UPDATE_INTERVAL 1

static bool segevict_initialized;

struct seg_evict_info evict;

#define IS_BAD_EVICT_CAN(seg)                                     \
    (seg->w_refcount > 0 || seg->evictable == 0 || (seg->create_at + seg->ttl) - time_proc_sec() < 5 || seg->next_seg_id == -1)


/* maybe we should use # of req instead of real time to make decision */
static inline bool
_should_rerank(void)
{
    bool rerank;
    proc_time_i curr_sec, prev_sec;
    curr_sec = time_proc_sec();
    prev_sec = __atomic_load_n(&evict.last_update_time, __ATOMIC_SEQ_CST);
    rerank = prev_sec == -1 || curr_sec - prev_sec > UPDATE_INTERVAL;

    rerank = rerank || evict.nseg - evict.idx_rseg < 8;

    if (rerank) {
        __atomic_store_n(&evict.last_update_time, curr_sec, __ATOMIC_SEQ_CST);
    }

    return rerank;
}


static inline int
_cmp_seg_FIFO(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    /* avoid segments that are currently being written to */
    if (IS_BAD_EVICT_CAN(seg1)) {
        return 1;
    }
    if (IS_BAD_EVICT_CAN(seg2)) {
        return -1;
    }


    return MAX(seg1->create_at, seg1->merge_at) -
            MAX(seg2->create_at, seg2->merge_at);
}

static inline int
_cmp_seg_CTE(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (IS_BAD_EVICT_CAN(seg1)) {
        return 1;
    }
    if (IS_BAD_EVICT_CAN(seg2)) {
        return -1;
    }

    return (seg1->create_at + seg1->ttl) - (seg2->create_at + seg2->ttl);
}

static inline int
_cmp_seg_util(const void *d1, const void *d2)
{
    struct seg *seg1 = &heap.segs[*(uint32_t *)d1];
    struct seg *seg2 = &heap.segs[*(uint32_t *)d2];

    if (IS_BAD_EVICT_CAN(seg1)) {
        return 1;
    }
    if (IS_BAD_EVICT_CAN(seg2)) {
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

    if (IS_BAD_EVICT_CAN(seg1)) {
        return 1;
    }
    if (IS_BAD_EVICT_CAN(seg2)) {
        return -1;
    }

#ifdef TRACK_ADVANCED_STAT
    return seg1->n_hit - seg2->n_hit;

    if (seg1->n_active == 0) {
        for (int i = 0; i < 131072; i++) {
            if (seg1->active_obj[i])
                seg1->n_active += 1;
        }
    }

    if (seg2->n_active == 0) {
        for (int i = 0; i < 131072; i++) {
            if (seg2->active_obj[i])
                seg2->n_active += 1;
        }
    }

    return seg1->n_active - seg2->n_active;
#else
    return 0;
#endif
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


//#ifdef TRACK_ADVANCED_STAT
//    static int n_rerank = 0;
//    n_rerank += 1;
//    if (n_rerank % 120 == 0) {
//        /* clear n_active */
//        for (int32_t i=0; i<heap.max_nseg; i++){
//            /* TODO (jason): not thread safe */
//            memset(heap.segs[i].active_obj, 0, sizeof(bool)*131072);
//            heap.segs[i].n_active = 0;
//        }
//    }
//#endif

    log_debug("ranked seg id %u %u %u %u %u %u %u %u %u %u ...",
            evict.ranked_seg_id[0], evict.ranked_seg_id[1],
            evict.ranked_seg_id[2], evict.ranked_seg_id[3],
            evict.ranked_seg_id[4], evict.ranked_seg_id[5],
            evict.ranked_seg_id[6], evict.ranked_seg_id[7],
            evict.ranked_seg_id[8], evict.ranked_seg_id[9]);
    seg_print(evict.ranked_seg_id[0]);
    seg_print(evict.ranked_seg_id[1]);
    seg_print(evict.ranked_seg_id[2]);
    seg_print(evict.ranked_seg_id[3]);
}


evict_rstatus_e
least_valuable_seg(int32_t *seg_id)
{
    struct seg *seg;

    if (evict.policy == EVICT_RANDOM) {
        uint32_t i = 0;
        *seg_id = rand() % heap.max_nseg;
        seg = &heap.segs[*seg_id];
        while ((IS_BAD_EVICT_CAN(seg)) && i <= heap.max_nseg) {
            /* transition to linear search */
            *seg_id = (*seg_id + 1) % heap.max_nseg;
            seg = &heap.segs[*seg_id];
            i++;
        }
        if (i == heap.max_nseg) {
            log_warn("unable to find a segment to evict");
            return EVICT_NO_SEALED_SEG;
        } else {
//            log_warn("%d %d %d %d", *seg_id, seg->seg_id, seg->next_seg_id, i);
//            seg_print_warn(seg->seg_id);
            return EVICT_OK;
        }
    } else {
        pthread_mutex_lock(&evict.mtx);

        _rank_seg();

        *seg_id = evict.ranked_seg_id[evict.idx_rseg++];
        seg = &heap.segs[*seg_id];

        /* it is OK if we read a staled seg.sealed because we will double check
         * when we perform real eviction */
        while (IS_BAD_EVICT_CAN(seg) && evict.idx_rseg < evict.nseg) {
            *seg_id = evict.ranked_seg_id[evict.idx_rseg++];
            seg = &heap.segs[*seg_id];
        }
        if (evict.idx_rseg >= evict.nseg) {
            seg = &heap.segs[evict.ranked_seg_id[0]];
            log_warn("unable to find a segment to evict, top seg %d, "
                     "ttl %d, accessible %d evictable %d, age %d, w_ref %d, "
                     "next_seg %d",
                    seg->seg_id, seg->ttl, seg->accessible, seg->evictable,
                    time_proc_sec() - seg->create_at, seg->w_refcount,
                    seg->next_seg_id);
            evict.idx_rseg = 0;
            /* better return a less utilized one */
            pthread_mutex_unlock(&evict.mtx);
            return EVICT_NO_SEALED_SEG;
        }

        pthread_mutex_unlock(&evict.mtx);

//        log_info("seg %d age %d, ttl %d, %d items, write offset %d, "
//                 "occupied size %d, n_hit %d, n_hit_last %d, n_active %d, "
//                 "evictable %d, accessible %d, ",
//                seg->seg_id, time_proc_sec() - seg->create_at,
//                seg->ttl, seg->n_item, seg->write_offset, seg->occupied_size,
//                seg->n_hit, seg->n_hit_last, seg->n_active, seg->evictable,
//                seg->accessible);

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
segevict_setup(evict_policy_e ev_policy, int32_t nseg)
{
    uint32_t i = 0;

    if (segevict_initialized) {
        log_warn("segevict has already initialized");
        segevict_teardown();
    }

    evict.last_update_time = -1;
    evict.policy = ev_policy;
    evict.nseg = nseg;
    evict.ranked_seg_id = cc_zalloc(sizeof(int32_t) * nseg);
    evict.idx_rseg = 0;
    pthread_mutex_init(&evict.mtx, NULL);

    for (i = 0; i < nseg; i++) {
        evict.ranked_seg_id[i] = i;
    }

    srand(time(NULL));
    segevict_initialized = true;
}
