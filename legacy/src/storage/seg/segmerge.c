
#include "seg.h"
#include "hashtable.h"
#include "item.h"
#include "segevict.h"
#include "ttlbucket.h"

#include <cc_mm.h>

#include <sys/types.h>

extern struct seg_evict_info evict_info;
extern struct ttl_bucket     ttl_buckets[MAX_N_TTL_BUCKET];
extern seg_metrics_st        *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];

static inline void
seg_copy(int32_t seg_id_dest, int32_t seg_id_src,
         double *cutoff_freq, double target_ratio);

int32_t
merge_segs(struct seg *segs_to_merge[],
           int n_evictable,
           double *merge_keep_ratio);

static inline uint64_t
n_evicted_seg(void)
{
    return __atomic_load_n(
        &seg_metrics->seg_evict_seg_cnt.counter, __ATOMIC_RELAXED);
}

static inline uint64_t
cal_mean_eviction_age(void)
{
    uint64_t evict_age_sum = __atomic_load_n(
        &seg_metrics->seg_evict_age_sum.counter, __ATOMIC_RELAXED);
    uint64_t evict_seg_cnt = __atomic_load_n(
        &seg_metrics->seg_evict_seg_cnt.counter, __ATOMIC_RELAXED);

    if (evict_seg_cnt == 0) {
        return 86400;
    }

    return evict_age_sum / evict_seg_cnt + evict_info.seg_mature_time;
}

/**
 * find n consecutive evictable segs starting from seg
 * currently only consider n=3, return NULL if cannot find any
 *
 * @param seg
 * @return
 */
static struct seg *
find_n_consecutive_evictable_seg(struct seg *seg)
{
    struct seg *next_seg1, *next_seg2;
    int32_t    seg_id, next_seg_id1, next_seg_id2;

    seg_id       = seg->seg_id;
    next_seg_id1 = heap.segs[seg_id].next_seg_id;
    next_seg_id2 =
        next_seg_id1 == -1 ? -1 : heap.segs[next_seg_id1].next_seg_id;

    while (seg_id != -1 && next_seg_id1 != -1 && next_seg_id2 != -1) {
        seg       = &heap.segs[seg_id];
        next_seg1 = &heap.segs[next_seg_id1];
        next_seg2 = &heap.segs[next_seg_id2];

        if (!seg_evictable(next_seg2)) {
            seg_id       = next_seg2->next_seg_id;
            next_seg_id1 = seg_id == -1 ? -1 : heap.segs[seg_id].next_seg_id;
            next_seg_id2 =
                next_seg_id1 == -1 ? -1 : heap.segs[next_seg_id1].next_seg_id;
            continue;
        }

        if (!seg_evictable(next_seg1)) {
            seg_id       = next_seg_id2;
            next_seg_id1 = next_seg2->next_seg_id;
            next_seg_id2 =
                next_seg_id1 == -1 ? -1 : heap.segs[next_seg_id1].next_seg_id;
            continue;
        }

        if (!seg_evictable(seg)) {
            seg_id       = next_seg_id1;
            next_seg_id1 = next_seg_id2;
            next_seg_id2 = next_seg2->next_seg_id;
            continue;
        }

        return seg;
    }

    return NULL;
}

/**
 * lock at most seg_n_max_merge segments to prevent other threads evicting
 */
static void
prep_seg_to_merge(int32_t start_seg_id,
                  struct seg *segs_to_merge[],
                  int *n_evictable_seg,
                  double *merge_keep_ratio)
{

    *n_evictable_seg = 0;
    int32_t    curr_seg_id = start_seg_id;
    struct seg *curr_seg;

    /* TODO(juncheng): do we need lock */
    pthread_mutex_lock(&heap.mtx);
    for (int i = 0; i < evict_info.merge_opt.seg_n_max_merge; i++) {
        if (curr_seg_id == -1) {
            break;
        }
        curr_seg = &heap.segs[curr_seg_id];
        if (!seg_evictable(curr_seg)) {
            break;
//            curr_seg_id = curr_seg->next_seg_id;
//            continue;
        }
        uint8_t evictable = __atomic_exchange_n(&curr_seg->evictable, 0, __ATOMIC_RELAXED);
#ifdef CC_ASSERT_PANIC
        ASSERT(evictable = 1);
#endif
        segs_to_merge[(*n_evictable_seg)++] = curr_seg;
        curr_seg_id = curr_seg->next_seg_id;
    }
    pthread_mutex_unlock(&heap.mtx);

    /* calculate how many bytes should be retained from each seg */
    int target_n_seg_to_merge = evict_info.merge_opt.seg_n_merge;
    if (*n_evictable_seg < target_n_seg_to_merge) {
        target_n_seg_to_merge = *n_evictable_seg;
    }

    for (int i = 0; i < evict_info.merge_opt.seg_n_max_merge; i++) {
        merge_keep_ratio[i] = 1.0 / target_n_seg_to_merge;
    }

    ASSERT(*n_evictable_seg > 1);
}

static inline void
replace_seg_in_chain(int32_t new_seg_id, int32_t old_seg_id)
{
    struct seg *new_seg = &heap.segs[new_seg_id];
    struct seg *old_seg = &heap.segs[old_seg_id];

    struct ttl_bucket *tb = &ttl_buckets[find_ttl_bucket_idx(old_seg->ttl)];

    /* all modification to seg chain needs to be protected by lock */
    ASSERT(pthread_mutex_trylock(&heap.mtx) != 0);

    int32_t prev_seg_id = old_seg->prev_seg_id;
    int32_t next_seg_id = old_seg->next_seg_id;

    if (prev_seg_id == -1) {
        ASSERT(tb->first_seg_id == old_seg_id);

        tb->first_seg_id = new_seg_id;
    }
    else {
        heap.segs[prev_seg_id].next_seg_id = new_seg_id;
    }

    ASSERT(next_seg_id != -1);
    heap.segs[next_seg_id].prev_seg_id = new_seg_id;

    new_seg->prev_seg_id = prev_seg_id;
    new_seg->next_seg_id = next_seg_id;
}


evict_rstatus_e
seg_merge_evict(int32_t *seg_id_ret)
{
    struct merge_opts *mopt = &evict_info.merge_opt;
    struct seg        *seg  = NULL;

    int32_t           bkt_idx;
    struct ttl_bucket *ttl_bkt;
    int               i;

    int32_t     seg_id;
    int32_t     first_seg_age;

    /* they are thread local because each thread keeps its own merge progress */
    static __thread int32_t last_bkt_idx = -1;
    if (last_bkt_idx == -1) {
        last_bkt_idx = rand() % MAX_N_TTL_BUCKET;
    }

    /* they thread local beacuse we would like to reduce memory allocations */
    static __thread struct seg **segs_to_merge   = NULL;
    static __thread double     *merge_keep_ratio = NULL;
    static __thread int32_t    n_evictable_seg;

    if (segs_to_merge == NULL) {
        segs_to_merge =
            cc_zalloc(sizeof(struct seg) * mopt->seg_n_max_merge);
        merge_keep_ratio = cc_zalloc(sizeof(double) * mopt->seg_n_max_merge);
    }

    /* we use MAX_N_TTL_BUCKET + 1 here because we start in the middle of
     * a segment chain (next_seg_to_merge), so it is possible there are no other
     * evictable segments except the ones early in the segment chain.
     *
     * For example,
     * if there is only one TTL bucket, we need to check this
     * ttl bucket again after reaching the end */
    for (i = 0; i < MAX_N_TTL_BUCKET + 1; i++) {
        bkt_idx = (last_bkt_idx + i) % MAX_N_TTL_BUCKET;
        ttl_bkt = &ttl_buckets[bkt_idx];
        if (ttl_buckets[bkt_idx].first_seg_id == -1) {
            /* empty TTL bucket */
            continue;
        }

        /* with more than 16 threads and 20% write, this lock becomes
         * the bottleneck;
         * as an alternative, we can try_lock,
         * but the problem is that because threads get seg at the same time,
         * if there are limited number of active TTL buckets,
         * only one thread will be able to evict; other threads will not
         * be able to get a new seg
         *
         * so for scalability, we need thread local active seg and
         * maintain a watermark on free seg and have
         * background thread to evict before every thread asks for a seg at
         * the same time, currently not implemented
         *
         * in NSDI work, we simply retry and return if an eviction fails
         */
        pthread_mutex_lock(&ttl_bkt->mtx);
//        if (pthread_mutex_trylock(&ttl_bkt->mtx) != 0) {
//            continue;
//        }

        seg = ttl_bkt->next_seg_to_merge != -1 ?
              &heap.segs[ttl_bkt->next_seg_to_merge] :
              &heap.segs[ttl_bkt->first_seg_id];

        seg = find_n_consecutive_evictable_seg(seg);
        if (seg == NULL) {
            /* cannot find enough evictable seg in this TTL bucket */
            ttl_buckets[bkt_idx].next_seg_to_merge = -1;
            seg_id        = ttl_buckets[bkt_idx].first_seg_id;
            if (seg_id != -1) {
                first_seg_age = time_proc_sec() - heap.segs[seg_id].create_at;
                if (heap.segs[seg_id].merge_at > 0) {
                    first_seg_age = time_proc_sec() - heap.segs[seg_id].merge_at;
                }
                /* the first segment in this bucket has not been evicted for a long time,
                 * this can happen if there is a corner case we have not considered,
                 * so evict it, one magic parameter here */
                bool seg_too_old = first_seg_age > (cal_mean_eviction_age() * 10);

                if (n_evicted_seg() > 100 && seg_too_old) {
                    bool success = rm_all_item_on_seg(seg_id, SEG_FORCE_EVICTION);
                    if (success) {
                        last_bkt_idx = bkt_idx + 1;
                        pthread_mutex_unlock(&ttl_bkt->mtx);

                        *seg_id_ret = seg_id;
                        return EVICT_OK;
                    }
                }
            }

            /* next ttl bucket please */
            pthread_mutex_unlock(&ttl_bkt->mtx);
            continue;
        }

        /* we have found enough consecutive evictable segments,
         * block the eviction of next seg_n_max_merge segments */
        prep_seg_to_merge(seg->seg_id, segs_to_merge,
            &n_evictable_seg, merge_keep_ratio);

        ttl_buckets[bkt_idx].next_seg_to_merge =
            merge_segs(segs_to_merge, n_evictable_seg, merge_keep_ratio);

        pthread_mutex_unlock(&ttl_bkt->mtx);

        last_bkt_idx = bkt_idx;

//        *seg_id_ret = seg_get_from_freepool(false);
//        log_warn("get seg %d", *seg_id_ret);
//        if (*seg_id_ret == -1)
//            return EVICT_NO_AVAILABLE_SEG;

        *seg_id_ret = segs_to_merge[0]->seg_id;
        return EVICT_OK;
    }

    /* reach here means we cannot find any segment to merge,
     * it might be 1. the mature time is too large
     * 2. there is limited number of active TTL buckets and the thread won't be
     * able to lock that bucket */
//    pthread_mutex_lock(&heap.mtx);
//    if (heap.n_free_seg > heap.n_reserved_seg) {
//        *seg_id_ret = seg_get_from_freepool(false);
//
//        pthread_mutex_unlock(&heap.mtx);
//        return EVICT_OK;
//    }
//    pthread_mutex_unlock(&heap.mtx);

    evict_info.seg_mature_time = evict_info.seg_mature_time / 2;

    log_warn("cannot find enough evictable segs");
    INCR(seg_metrics, seg_evict_ex);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    dump_seg_info();
#endif

    return EVICT_NO_AVAILABLE_SEG;
}

static void
seg_copy(int32_t seg_id_dest, int32_t seg_id_src,
         double *cutoff_freq, double target_ratio)
{
    struct merge_opts *mopt          = &evict_info.merge_opt;

    struct item *it = NULL, *last_it = NULL;

    struct seg *seg_dest = &heap.segs[seg_id_dest];
    struct seg *seg_src  = &heap.segs[seg_id_src];

    int32_t seg_id_src_ht = seg_id_src;
    int32_t seg_id_dest_ht = seg_id_dest;
#ifdef DEBUG_MODE
    /* hash table uses non_decr seg id when debug */
    seg_id_src_ht = heap.segs[seg_id_src].seg_id_non_decr;
    seg_id_dest_ht = heap.segs[seg_id_dest].seg_id_non_decr;
#endif

    uint8_t *seg_data_src  = get_seg_data_start(seg_id_src);
    uint8_t *seg_data_dest = get_seg_data_start(seg_id_dest);
    uint8_t *curr_src      = seg_data_src;

    uint32_t offset = MIN(seg_src->write_offset, heap.seg_size) - ITEM_HDR_SIZE;

    int32_t it_sz, it_offset;
    double  it_freq;

    bool it_up_to_date;
    bool dest_seg_full = false;

    /* if the merged seg has reached stop_byte, no more new seg will be merged
 * into it, so let's copy more from current seg to the merged seg */
    bool              copy_all_items = false;
    if (*cutoff_freq < 0.0001) {
        /* the passed in cutoff_freq is 0, indicating previous segments have
         * almost no bytes copied */
        copy_all_items = true;
    }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *) (seg_data_dest) == SEG_MAGIC);
    ASSERT(*(uint64_t *) (curr_src) == SEG_MAGIC);
    curr_src += sizeof(uint64_t);
#endif

    int    n_scanned    = 0, n_copied = 0;
    double mean_size    = (double) seg_src->live_bytes / seg_src->n_live_item;
    double cutoff       = (1 + *cutoff_freq) / 2;
    int    update_intvl = (int) heap.seg_size / 10;
    int    n_th_update  = 1;

    while (curr_src - seg_data_src < offset) {
        last_it = it;
        it      = (struct item *) curr_src;

        if (it->klen == 0 && it->vlen == 0) {
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
            ASSERT(__atomic_load_n(&it->magic, __ATOMIC_SEQ_CST) == 0);
#endif
            if (seg_src->n_live_item > 0) {
                log_warn("seg %d: end of merge: %d items left",
                    seg_id_src, seg_src->n_live_item);
#if defined(CC_ASSERT_PANIC)
                scan_hashtable_find_seg(seg_id_src_ht);
#endif
            }
            break;
        }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC);
#endif

        it_sz = item_ntotal(it);
        n_scanned += it_sz;
        if (n_scanned >= n_th_update * update_intvl) {
            n_th_update += 1;
            /* currently magic, needs a principled way */
            double t = (((double) n_copied) / n_scanned - target_ratio)
                / target_ratio;
            if (t > 0.5 || t < -0.5) {
                cutoff = cutoff * (1 + t);
            }
        }

        /* we will not merge a new segment because the merged seg has more than
         * stop_bytes, and current seg has less than seg_size - stop_bytes bytes
         * left, so let's copy all,
         * most of the time, the impact of this is small */
        if (!copy_all_items
            && seg_dest->write_offset >= mopt->stop_bytes
            && curr_src - seg_data_src > mopt->stop_bytes) {

            copy_all_items = true;
            log_verb("seg copy %d %d/%d, last item sz %d",
                seg_id_src, curr_src - seg_data_src,
                seg_dest->write_offset, item_ntotal(last_it));
        }

        if (it->deleted) {
            /* this is necessary for current hash table design */
            hashtable_evict(item_key(it), item_nkey(it),
                            seg_id_src_ht, curr_src - seg_data_src);
            curr_src += it_sz;
            continue;
        }

        it_offset = curr_src - seg_data_src;

#ifdef STORE_FREQ_IN_HASHTABLE
        it_freq = (double) hashtable_get_it_freq(item_key(it), it->klen,
            seg_id_src_ht, it_offset);
#else
        it_freq = (double) it->freq;
#endif
        ASSERT(it_freq >= 0);
        it_freq = it_freq / ((double) it_sz / mean_size);

        if (it_freq <= cutoff && (!copy_all_items)) {
            DECR_N(seg_metrics, item_curr_bytes, it_sz);
            DECR(seg_metrics, item_curr);
            hashtable_evict(item_key(it), it->klen, seg_id_src_ht, it_offset);
            curr_src += it_sz;
            continue;
        }

        if (seg_dest->write_offset + it_sz > heap.seg_size) {
            if (!dest_seg_full) {
                dest_seg_full = true;
                log_debug("copy from seg %" PRId32 " to seg %" PRId32
                    ", destination seg full %d + %d src offset %d",
                    seg_id_src, seg_id_dest, seg_dest->write_offset, it_sz,
                    it_offset);
            }

            hashtable_evict(item_key(it), it->klen, seg_id_src_ht, it_offset);
            curr_src += it_sz;
            continue;
        }

            /* we will retain this object, first copy the data, then relink in
             * the hashtable */
#ifdef USE_PMEM
            pmem_memcpy_nodrain(seg_data_dest + seg_dest->write_offset, curr_src, it_sz);
#else
        memcpy(seg_data_dest + seg_dest->write_offset, curr_src, it_sz);
#endif

        it_up_to_date = hashtable_relink_it(item_key(it), it->klen,
            seg_id_src_ht, it_offset, seg_id_dest_ht, seg_dest->write_offset);

        if (it_up_to_date) {
            /* we need atomics because we already copied data on seg_dest
             * can be removed or updated */
            __atomic_fetch_add(&seg_dest->write_offset, it_sz, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_dest->total_bytes, it_sz, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_dest->live_bytes, it_sz, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_dest->n_total_item, 1, __ATOMIC_RELAXED);
            __atomic_fetch_add(&seg_dest->n_live_item, 1, __ATOMIC_RELAXED);
            n_copied += it_sz;
        }

        curr_src += it_sz;
    }

    /* using this one will crash, there must be some data race which incr
     * n_live_item somewhere
     */
#ifdef DEBUG_MODE
    if (seg_src->n_live_item > 0) {
//    if (seg_src->n_rm_item != seg_src->n_total_item) {
        log_warn("seg %d after merge %d items left", seg_src->seg_id, seg_src->n_live_item);
        scan_hashtable_find_seg(seg_id_src_ht);
        ASSERT(0);
    }
#endif

    *cutoff_freq = cutoff;
    log_verb("move items from seg %d to seg %d, new seg %d items, offset %d, "
             "cutoff %.2lf, target ratio %.2lf",
        seg_id_src, seg_id_dest, seg_dest->n_live_item, seg_dest->write_offset,
        *cutoff_freq, target_ratio);
}

/* merge at most n_evictable consecutive segs into one seg,
 * from each seg, we retain merge_keep_bytes,
 * if the merged seg is full return earlier
 *
 * return the number of segs that are merged
 *
 **/
int32_t
merge_segs(struct seg *segs_to_merge[],
           int n_evictable,
           double *merge_keep_ratio)
{
    INCR(seg_metrics, seg_merge);

    struct merge_opts *mopt = &evict_info.merge_opt;

    static int empty_merge      = 0;
    static int successful_merge = 0;

    int32_t    curr_seg_id;
    struct seg *curr_seg;
    uint8_t    accessible;
    int        n_merged         = 0;

    /* this is the next seg_id of the last evictable segment, we keep it
     * in case there are no active objects in all evictable segments (so no merged seg),
     * we return this seg */
    int32_t last_seg_next_seg_id = segs_to_merge[n_evictable - 1]->next_seg_id;

    /* get a reserved seg as the new seg for storing the copied objects */
    int32_t new_seg_id = seg_get_from_freepool(true);
    seg_init(new_seg_id);

    struct seg *new_seg = &heap.segs[new_seg_id];
    ASSERT(new_seg->evictable == 0);

    new_seg->create_at   = segs_to_merge[0]->create_at;
    new_seg->merge_at    = time_proc_sec();
    new_seg->ttl         = segs_to_merge[0]->ttl;
    new_seg->accessible  = 1;
    new_seg->prev_seg_id = segs_to_merge[0]->prev_seg_id;

    /* if the request cnt of an object / (obj_size/mean_obj_size) > cutoff
     * we retain the object, the cutoff freq is adjusted during the merge
     * to make sure we retain specified number of bytes
     * TODO(junchengy): figure out a better cutoff frequency estimation */
    double cutoff_freq = 1;
    /* if all of the n seg have no active objects, we will have an empty merge,
     * it can happen if the workload shows scan pattern or the cutoff frequency
     * is too high, we reset cutoff frequency if this happens */
    if (empty_merge > successful_merge && empty_merge > 2) {
        cutoff_freq = 0;
    }

    /* start from start_seg until new_seg is full or no seg can be merged */
    while (new_seg->write_offset < mopt->stop_bytes && n_merged < n_evictable) {
        curr_seg    = segs_to_merge[n_merged];
        curr_seg_id = curr_seg->seg_id;

        seg_copy(new_seg_id, curr_seg_id, &cutoff_freq,
            merge_keep_ratio[n_merged]);

        /* remove the evicted seg and return to freepool */
        accessible =
            __atomic_exchange_n(&(curr_seg->accessible), 0, __ATOMIC_RELAXED);
        ASSERT(accessible == 1);

        seg_wait_refcnt(curr_seg_id);

        pthread_mutex_lock(&heap.mtx);
        if (n_merged == 0) {
            /* place the new seg at the position of the first evicted seg and
             * not return this seg to freepool, keep it for the immediate use */
            replace_seg_in_chain(new_seg_id, curr_seg_id);
        }
        else {
            rm_seg_from_ttl_bucket(curr_seg_id);
            seg_add_to_freepool(curr_seg_id, SEG_EVICTION);
        }

        pthread_mutex_unlock(&heap.mtx);

        n_merged++;

        INCR_N(seg_metrics, seg_evict_age_sum,
            time_proc_sec() - curr_seg->create_at);
        INCR(seg_metrics, seg_evict_seg_cnt);
    }

    ASSERT(n_merged > 0);

    if (new_seg->live_bytes <= 8) {
        /* if the evicted segs all have no live object */
        new_seg->accessible = 0;

        pthread_mutex_lock(&heap.mtx);
        rm_seg_from_ttl_bucket(new_seg_id);
        seg_add_to_freepool(new_seg_id, SEG_EVICTION);
        pthread_mutex_unlock(&heap.mtx);

        log_warn("merged %d segments with no active objects, "
                 "return reserved seg %d", n_merged, new_seg_id);
        for (int i = 0; i < n_merged; i++) {
            SEG_PRINT(segs_to_merge[i]->seg_id, "seg info", log_debug);
        }

        empty_merge += 1;

        return last_seg_next_seg_id;
    }
    else {
        /* because we locked n_evictable segs,
         * and we have only evicted n_merged segs,
         * change the status of un-merged seg */
        for (int i = n_merged; i < n_evictable; i++) {
            uint8_t evictable = __atomic_exchange_n(
                &segs_to_merge[i]->evictable, 1, __ATOMIC_RELAXED);
            ASSERT(evictable == 0);
        }

        /* because of internal memory fragmentation, the seg is not always full
         * set the part that written to 0 */
        memset(get_seg_data_start(new_seg_id) + new_seg->write_offset,
            0, heap.seg_size - new_seg->write_offset);
        __atomic_store_n(&new_seg->evictable, 1, __ATOMIC_RELAXED);
        successful_merge += 1;

        /* print stat */
        char     merged_segs[1024];
        int      pos       = 0;
        for (int i         = 0; i < n_merged; i++) {
            pos += snprintf(merged_segs + pos, 1024 - pos, "%d, ",
                segs_to_merge[i]->seg_id);
        }
        log_debug("ttl %d, merged %d/%d segs (%s) to seg %d, "
                  "curr #free segs %d, new seg offset %d, occupied size %d, "
                  "%d items",
            new_seg->ttl, n_merged, n_evictable, merged_segs, new_seg_id,
            heap.n_free_seg, new_seg->write_offset,
            new_seg->live_bytes, new_seg->n_live_item);

        log_verb("***************************************************");

        return heap.segs[new_seg_id].next_seg_id;
    }

    ASSERT(0);
}



