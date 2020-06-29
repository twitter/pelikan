#pragma once

#include "constant.h"
//#include "seg.h"

#include <time/time.h>
#include <cc_mm.h>

typedef enum {
    EVICT_NONE = 0,
    EVICT_FIFO,
    EVICT_TTL,
    EVICT_UTIL,
    EVICT_SMART,

    EVICT_INVALID
} evict_policy_e;


#ifdef do_not_define
//static struct seg *
//_seg_table_rand(void)
//{
//    uint32_t rand_idx;
//
//    rand_idx = (uint32_t) rand() % heap.nseg;
//    return heap.seg_table[rand_idx];
//}
//
//static struct seg *
//_seg_lruq_head(void)
//{
//    return TAILQ_FIRST(&heap.seg_lruq);
//}
//
//static void
//_seg_lruq_append(struct seg *seg)
//{
//    log_vverb("append seg %p with id %d from lruq", seg, seg->id);
//    TAILQ_INSERT_TAIL(&heap.seg_lruq, seg, s_tqe);
//}
//
//static void
//_seg_lruq_remove(struct seg *seg)
//{
//    log_vverb("remove seg %p with id %d from lruq", seg, seg->id);
//    TAILQ_REMOVE(&heap.seg_lruq, seg, s_tqe);
//}

/*
 * Get a random seg from all active segs and evict it for new allocation.
 *
 * Note that the seg_table enables us to have O(1) lookup for every seg in
 * the system. The inserts into the table are just appends - O(1) and there
 * are no deletes from the seg_table. These two constraints allows us to keep
 * our random choice uniform.
 */
static struct seg *
_seg_evict_rand(void)
{
    struct seg *seg;
    int        i = 0;

    do {
        seg = _seg_table_rand();
    } while (seg != NULL && ++i < TRIES_MAX && !_seg_check_no_refcount(seg));

    if (seg == NULL) {
        /* warning here because eviction failure should be rare. This can
         * indicate there are dead/idle connections hanging onto items and
         * seg refcounts.
         */
        log_warn("can't find a seg for random-evicting seg with %d tries", i);
    }
    else {
        log_verb("random-evicting seg %p with id %u", seg, seg->id);
        _seg_evict_one(seg);
    }

    return seg;
}

/*
 * Evict by looking into least recently used queue of all segs.
 */
static struct seg *
_seg_evict_lru(int id)
{
    struct seg *seg = _seg_lruq_head();
    int        i    = 0;

    while (seg != NULL && ++i < TRIES_MAX && !_seg_check_no_refcount(seg)) {
        seg = TAILQ_NEXT(seg, s_tqe);
    };

    if (seg == NULL) {
        /* warning here because eviction failure should be rare. This can
         * indicate there are dead/idle connections hanging onto items and
         * seg refcounts.
         */
        log_warn("can't find a seg for lru-evicting seg with %d tries", i);
    }
    else {
        log_verb("lru-evicting seg %p with id %u", seg, seg->id);
        _seg_evict_one(seg);
    }

    return seg;
}

/*
 * Evict a seg by evicting all the items within it. This means that the
 * items that are carved out of the seg must either be deleted from their
 * a) hash + lru Q, or b) free Q. The candidate seg itself must also be
 * delinked from its respective seg pool so that it is available for reuse.
 *
 * Eviction complexity is O(#items/seg).
 */
static void
_seg_evict_one(struct seg *seg)
{
    struct segclass *p;
    struct item     *it;
    uint32_t        i;

    p = &segclass[seg->id];

    INCR(seg_metrics, seg_evict);

    /* candidate seg is also the current seg */
    if (p->next_item_in_seg != NULL
        && seg == item_to_seg(p->next_item_in_seg)) {
        p->nfree_item       = 0;
        p->next_item_in_seg = NULL;
    }

    /* delete seg items either from hash or free Q */
    for (i = 0; i < p->nitem; i++) {
        it = _seg_to_item(seg, i, p->size);

        if (it->is_linked) {
            it->is_linked = 0;
            hashtable_delete(item_key(it), it->klen, hash_table);
        }
        else if (it->in_freeq) {
            ASSERT(seg == item_to_seg(it));
            ASSERT(!SLIST_EMPTY(&p->free_itemq));
            ASSERT(p->nfree_itemq > 0);

            it->in_freeq = 0;
            p->nfree_itemq--;
            SLIST_REMOVE(&p->free_itemq, it, item, i_sle);
        }
    }

    /* unlink the seg from its class */
    _seg_lruq_remove(seg);
}
#endif


struct seg_evict_info {
    evict_policy_e policy;
    proc_time_i last_update_time;
    uint32_t max_nseg_dram;
    uint32_t max_nseg_pmem;
    uint32_t *ranked_seg_id_dram;  /* the least valuable to the most valuable */
    uint32_t *ranked_seg_id_pmem;
};


/**
 * find the least valuable segment in DRAM
 * return seg_id
 */
uint32_t
least_valuable_seg_dram(void);

/**
 * find the least valuable segment in DRAM
 * return seg_id
 */
uint32_t
least_valuable_seg_pmem(void);


/* this must be setup update seg_setup has finished */
void segevict_setup(evict_policy_e ev_policy, uint32_t nseg_dram, uint32_t nseg_pmem);

void segevict_teardown(void);
