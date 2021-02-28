
#include "segmerge.h"
#include "seg.h"
//#include "constant.h"
#include "hashtable.h"
#include "item.h"
#include "segevict.h"
#include "ttlbucket.h"
//#include "datapool/datapool.h"

#include <cc_mm.h>
//#include <cc_util.h>



extern struct seg_evict_info evict_info;
extern struct ttl_bucket     ttl_buckets[MAX_N_TTL_BUCKET];
extern seg_metrics_st        *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];

static int64_t n_merge_seg       = 0;
static int64_t merge_seg_age_sum = 0;

static inline void
seg_copy(int32_t seg_id_dest, int32_t seg_id_src,
         double *cutoff_freq, double target_ratio);

static inline void
prep_seg_to_merge(int32_t start_seg_id,
                  struct seg *segs_to_merge[], int *n_seg_to_merge,
                  double *merge_keep_ratio);

static inline void
replace_seg_in_chain(int32_t new_seg_id, int32_t old_seg_id);

int32_t
merge_segs(struct seg *segs_to_merge[], int at_most_n_seg);

bool
check_merge_seg(void)
{
    struct seg *seg  = NULL, *next1_seg, *next2_seg = NULL;

    int32_t           ttl_bkt_idx;
    struct ttl_bucket *ttl_bkt;
    int               i;
    bool       found = false;

    static __thread int32_t    last_ttl_bkt_idx  = 0;
    static __thread struct seg **segs_to_merge   = NULL;
    static __thread double     *merge_keep_ratio = NULL;
    static __thread int32_t    at_most_n_seg;

    if (segs_to_merge == NULL) {
        segs_to_merge    =
            cc_zalloc(
                sizeof(struct seg) * evict_info.merge_opt.seg_n_max_merge);
        merge_keep_ratio =
            cc_zalloc(sizeof(double) * evict_info.merge_opt.seg_n_max_merge);
    }

    pthread_mutex_lock(&heap.mtx);
    if (heap.n_free_seg > heap.n_reserved_seg) {
        pthread_mutex_unlock(&heap.mtx);
        return true;
    }
    pthread_mutex_unlock(&heap.mtx);

    int n_retry = -1;
    test:
    n_retry += 1;
    /* it is important to have MAX_N_TTL_BUCKET+1, because
     * if there is only one TTL bucket, we need to check this
     * ttl bucket again after reaching the end of bucket */
    for (i      = 0; i < MAX_N_TTL_BUCKET + 1; i++) {
        ttl_bkt_idx = (last_ttl_bkt_idx + i) % MAX_N_TTL_BUCKET;
        ttl_bkt = &ttl_buckets[ttl_bkt_idx];
        if (ttl_buckets[ttl_bkt_idx].first_seg_id == -1) {
            continue;
        }

        if (pthread_mutex_trylock(&ttl_bkt->mtx) != 0) {
            /* with more than 16 threads and 20% write, this lock becomes
             * the bottleneck, so for scalability, we just check next TTL bucket
             */
            continue;
        }

        if (ttl_bkt->next_seg_to_merge != -1) {
            seg = &heap.segs[ttl_bkt->next_seg_to_merge];
        }
        else {
            seg = &heap.segs[ttl_bkt->first_seg_id];
        }

        while (1) {
            if (seg->next_seg_id == -1) {
                break;
            }
            next1_seg = &heap.segs[seg->next_seg_id];

            if (next1_seg->next_seg_id == -1) {
                break;
            }
            next2_seg = &heap.segs[next1_seg->next_seg_id];

            if (next2_seg->next_seg_id == -1) {
                break;
            }

            if (seg_evictable(seg)) {
                if (seg_evictable(next1_seg)) {
                    if (seg_evictable(next2_seg)) {
                        found = true;
                        break;
                    }
                    else {
                        if (next2_seg->next_seg_id != -1) {
                            seg = &heap.segs[next2_seg->next_seg_id];
                            continue;
                        }
                        else {
                            break;
                        }
                    }
                }
                else {
                    seg = next2_seg;
                    continue;
                }
            }
            else {
                seg = next1_seg;
                continue;
            }
        }

        if (!found) {
            ttl_buckets[ttl_bkt_idx].next_seg_to_merge = -1;
            int32_t      seg_id        = ttl_buckets[ttl_bkt_idx].first_seg_id;
            delta_time_i first_seg_age = time_proc_sec() -
                heap.segs[seg_id].create_at;
            /* the segments in this bucket cannot be merged, but it has been
             * too old, we evict it */


            if (n_merge_seg > 100
                && first_seg_age > (merge_seg_age_sum / n_merge_seg) * 2) {
                int success = rm_all_item_on_seg(seg_id, SEG_EXPIRATION);
                if (success) {
                    pthread_mutex_lock(&heap.mtx);
                    seg_add_to_freepool(seg_id, SEG_EVICTION);
                    pthread_mutex_unlock(&heap.mtx);
                    last_ttl_bkt_idx = ttl_bkt_idx + 1;
                    pthread_mutex_unlock(&ttl_bkt->mtx);
                    return true;
                }
            }


            /* next ttl bucket please */
            pthread_mutex_unlock(&ttl_bkt->mtx);
            continue;
        }


        /* block the eviction of next seg_n_max_merge segments */
        prep_seg_to_merge(seg->seg_id, segs_to_merge, &at_most_n_seg,
            merge_keep_ratio);
        pthread_mutex_unlock(&ttl_bkt->mtx);

        /* I hope I can move the ttl_bkt lock out of merge_segs
         * it is not clear when it is in the merge_segs */
        ttl_buckets[ttl_bkt_idx].next_seg_to_merge =
            merge_segs(segs_to_merge, at_most_n_seg);
        last_ttl_bkt_idx = ttl_bkt_idx;

        return true;
    }

//    for (int j=0; j<heap.max_nseg; j++) {
//        seg_print_warn(j);
//        log_warn("%d mergeable %d", j, seg_evictable(&heap.segs[j]));
//    }
    log_warn("cannot find mergeable seg, retry %d", n_retry);

//    for (i = 0; i < MAX_N_TTL_BUCKET+1; i++) {
//        if (ttl_buckets[i].first_seg_id == -1)
//            continue;
//        seg = &heap.segs[ttl_buckets[i].first_seg_id];
//        while (seg != NULL) {
//            printf("seg %d (%d), ", seg->seg_id, seg_evictable(seg));
//            if (seg->next_seg_id != -1)
//                seg = &heap.segs[seg->next_seg_id];
//            else
//                seg = NULL;
//        }
//        printf("\n");
//    }
    if (n_retry < 8) {
        usleep(200 * n_retry * n_retry);
        evict_info.seg_mature_time = evict_info.seg_mature_time / 2;
        goto test;
    }

    char    s[64];
    int32_t j;
    for (j = 0; j < heap.max_nseg; j++) {
        snprintf(s, 64, "%d mergeable %d", j, seg_evictable(&heap.segs[j]));
        SEG_PRINT(j, s, log_warn);
    }
//    for (i = 0; i < MAX_N_TTL_BUCKET+1; i++) {
//        if (ttl_buckets[i].first_seg_id == -1)
//            continue;
//        seg = &heap.segs[ttl_buckets[i].first_seg_id];
//        while (seg != NULL) {
//            printf("seg %d (%d), ", seg->seg_id, seg_evictable(seg));
//            if (seg->next_seg_id != -1)
//                seg = &heap.segs[seg->next_seg_id];
//            else
//                seg = NULL;
//        }
//        printf("\n");
//    }
    ASSERT(0);
    return false;
}

static inline void
seg_copy(int32_t seg_id_dest, int32_t seg_id_src,
         double *cutoff_freq, double target_ratio)
{
    struct item *it = NULL;
    struct item *last_it       = NULL;
    struct seg  *seg_dest      = &heap.segs[seg_id_dest];
    struct seg  *seg_src       = &heap.segs[seg_id_src];
    uint8_t     *seg_data_src  = get_seg_data_start(seg_id_src);
    uint8_t     *curr_src      = seg_data_src;

    uint8_t  *seg_data_dest = get_seg_data_start(seg_id_dest);
    uint32_t offset         =
                 MIN(seg_src->write_offset, heap.seg_size) - ITEM_HDR_SIZE;

    int32_t it_sz = 0;
    bool item_up_to_date;
    bool seg_in_full = false;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(*(uint64_t *) (seg_data_dest) == SEG_MAGIC);
    ASSERT(*(uint64_t *) (curr_src) == SEG_MAGIC);
    curr_src += sizeof(uint64_t);
#endif

    bool        copy_all_items = false;
    if (*cutoff_freq < 0.0001) {
        copy_all_items = true;
    }

    int    n_scanned    = 0, n_copied = 0;
    double mean_size    = (double) seg_src->occupied_size / seg_src->n_item;
    double cutoff       = (1 + *cutoff_freq) / 2;
    int    update_intvl = (int) heap.seg_size / 10;
    int    n_th_update  = 1;

    double hit;

    while (curr_src - seg_data_src < offset) {
        last_it = it;
        it      = (struct item *) curr_src;

        if (it->klen == 0 && it->vlen == 0) {
            break;
        }

        ASSERT(seg_src->n_item >= 0);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(it->magic == ITEM_MAGIC);
#endif

        it_sz = item_ntotal(it);
        n_scanned += it_sz;
        if (n_scanned >= n_th_update * update_intvl) {
            n_th_update += 1;
//            double t = (double) n_copied/n_scanned - target_ratio;
//            if (t > 0.1 || t < -0.1) {
            /* new change */
            double t = (((double) n_copied) / n_scanned - target_ratio)
                / target_ratio;
            if (t > 0.5 || t < -0.5) {
                cutoff = cutoff * (1 + t);
            }
        }

        /* we will not merge a new segment, so let's copy all items left,
         * most of the time, the impact of this is small */
        if (!copy_all_items
            && (seg_dest->write_offset >= evict_info.merge_opt.stop_bytes)
            && curr_src - seg_data_src > evict_info.merge_opt.stop_bytes) {
            copy_all_items = true;
            log_verb("seg copy %d %d/%d, last item sz %d", seg_id_src,
                curr_src - seg_data_src,
                seg_dest->write_offset, item_ntotal(last_it));
        }

        if (it->deleted) {
            curr_src += it_sz;
            continue;
        }

        hit = hashtable_get_it_freq(item_key(it), it->klen, seg_id_src,
            curr_src - seg_data_src);

        hit = (double) hit / ((double) it_sz / mean_size);

        if (hit <= cutoff && (!copy_all_items)) {
            hashtable_evict(item_key(it), it->klen, seg_id_src,
                curr_src - seg_data_src);
            curr_src += it_sz;
            continue;
        }

        if (seg_dest->write_offset + it_sz > heap.seg_size) {
            /* TODO(jason): add a new metric */
            if (!seg_in_full) {
                seg_in_full = true;
                log_debug("copy from seg %" PRId32 " to seg %" PRId32
                    ", destination seg full %d + %d src offset %d",
                    seg_id_src, seg_id_dest, seg_dest->write_offset, it_sz,
                    curr_src - seg_data_src);
            }

            hashtable_evict(item_key(it), it->klen, seg_id_src,
                curr_src - seg_data_src);
            curr_src += it_sz;
            continue;
        }

            /* first copy data */
#ifdef USE_PMEM
            pmem_memcpy_nodrain(seg_data_dest + seg_dest->write_offset, curr_src, it_sz);
#else
        memcpy(seg_data_dest + seg_dest->write_offset, curr_src, it_sz);
#endif

        /* try to relink */
        item_up_to_date = hashtable_relink_it(item_key(it), it->klen,
            seg_id_src, curr_src - seg_data_src, seg_id_dest,
            seg_dest->write_offset);

        if (item_up_to_date) {
            seg_dest->write_offset += it_sz;
            seg_dest->occupied_size += it_sz;
            seg_dest->n_item += 1;
            seg_src->n_item -= 1;
            n_copied += it_sz;
        }

        curr_src += it_sz;
    }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    if (seg_src->n_item != 0) {
        log_warn("after copy %d items left", seg_src->n_item);
        scan_hashtable_find_seg(seg_id_src);
    }
#endif

    *cutoff_freq = cutoff;
    log_debug(
        "move items from seg %d to seg %d, new seg %d items, offset %d, cutoff %.2lf, target ratio %.2lf",
        seg_id_src, seg_id_dest, seg_dest->n_item, seg_dest->write_offset,
        *cutoff_freq, target_ratio);
}

/**
 * lock at most seg_n_max_merge segments to prevent other threads evicting
 */
static inline void
prep_seg_to_merge(int32_t start_seg_id,
                  struct seg *segs_to_merge[], int *n_seg_to_merge,
                  double *merge_keep_ratio)
{

    *n_seg_to_merge = 0;
    int32_t    curr_seg_id = start_seg_id;
    struct seg *curr_seg;

    uint8_t evictable;

    pthread_mutex_lock(&heap.mtx);
    for (int i = 0; i < evict_info.merge_opt.seg_n_max_merge; i++) {
        if (curr_seg_id == -1) {
            /* this could happen when prev seg is evicted */
            break;
        }
        curr_seg  = &heap.segs[curr_seg_id];
        if (!seg_evictable(curr_seg)) {
            curr_seg_id = curr_seg->next_seg_id;
            continue;
        }
        evictable = __atomic_exchange_n(&curr_seg->evictable, 0,
            __ATOMIC_RELAXED);
        if (evictable == 0) {
            /* concurrent merge and evict */
            curr_seg_id = curr_seg->next_seg_id;
            continue;
        }
        segs_to_merge[(*n_seg_to_merge)++] = curr_seg;
        __atomic_fetch_add(&n_merge_seg, 1, __ATOMIC_RELAXED);
        __atomic_fetch_add(&merge_seg_age_sum, proc_sec - curr_seg->create_at,
            __ATOMIC_RELAXED);
        curr_seg_id = curr_seg->next_seg_id;
    }
    pthread_mutex_unlock(&heap.mtx);

    ASSERT(*n_seg_to_merge > 1);
}

static inline void
replace_seg_in_chain(int32_t new_seg_id, int32_t old_seg_id)
{
    struct seg        *new_seg = &heap.segs[new_seg_id];
    struct seg        *old_seg = &heap.segs[old_seg_id];
    struct ttl_bucket *tb      =
                          &ttl_buckets[find_ttl_bucket_idx(old_seg->ttl)];

    /* all modification to seg list needs to be protected by lock */
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

/* merge at most n_seg consecutive segs into one seg,
 * if the merged seg is full return earlier
 *
 * the return value indicates how many segs are merged
 *
 **/
int32_t
merge_segs(struct seg *segs_to_merge[], int at_most_n_seg)
{
    struct merge_opts *mopt = &evict_info.merge_opt;

    static int empty_merge      = 0;
    static int successful_merge = 0;

    int32_t    curr_seg_id;
    struct seg *curr_seg;
    uint8_t    accessible;
    int        n_merged         = 0;

    /* this is the next seg_id of the last segment, we keep copy of it
     * in case there are no active objects in all these segments,
     * this is the return value */
    int32_t
        last_seg_next_seg_id = segs_to_merge[at_most_n_seg - 1]->next_seg_id;

    /* prepare new seg */
    int32_t new_seg_id = seg_get_from_freepool(true);
    seg_init(new_seg_id);

    struct seg *new_seg = &heap.segs[new_seg_id];
    ASSERT(new_seg->evictable == 0);

    new_seg->create_at   = segs_to_merge[0]->create_at;
    new_seg->merge_at    = time_proc_sec();
    new_seg->ttl         = segs_to_merge[0]->ttl;
    new_seg->accessible  = 1;
    new_seg->prev_seg_id = segs_to_merge[0]->prev_seg_id;
    double cutoff_freq = 1;
    if (empty_merge > successful_merge && empty_merge > 2) {
        cutoff_freq = 0;
    }

    /* start from start_seg until new_seg is full or no seg can be merged */
    /* TODO: update stop ratio to stop byte */
    while (new_seg->write_offset < heap.seg_size * mopt->stop_ratio
        && n_merged < at_most_n_seg) {

        curr_seg    = segs_to_merge[n_merged++];
        curr_seg_id = curr_seg->seg_id;

        seg_copy(new_seg_id, curr_seg_id, &cutoff_freq,
            mopt->target_ratio);
        accessible = __atomic_exchange_n(&(curr_seg->accessible), 0,
            __ATOMIC_RELAXED);
        ASSERT(accessible == 1);

        seg_wait_refcnt(curr_seg_id);
        pthread_mutex_lock(&heap.mtx);
        if (n_merged - 1 == 0) {
            replace_seg_in_chain(new_seg_id, curr_seg_id);
        }
        else {
            rm_seg_from_ttl_bucket(curr_seg_id);
        }

        seg_add_to_freepool(curr_seg_id, SEG_EVICTION);
        pthread_mutex_unlock(&heap.mtx);
    }

    ASSERT(n_merged > 0);

    /* if no seg has active object */
    if (new_seg->occupied_size <= 8) {
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
        /* changed the status of un-merged seg */
        for (int i = n_merged; i < at_most_n_seg; i++) {
            uint8_t evictable = __atomic_exchange_n(
                &segs_to_merge[i]->evictable, 1, __ATOMIC_RELAXED);
            ASSERT(evictable == 0);
        }

        /* in seg_copy, we could copy over unused bytes */
        memset(get_seg_data_start(new_seg_id) + new_seg->write_offset,
            0, heap.seg_size - new_seg->write_offset);
        new_seg->evictable = 1;

        /* print stat */
        char     merged_segs[1024];
        int      pos       = 0;
        for (int i         = 0; i < n_merged; i++) {
            pos += snprintf(merged_segs + pos, 1024 - pos, "%d, ",
                segs_to_merge[i]->seg_id);
        }
        log_info("ttl %d, merged %d/%d segs (%s) to seg %d, "
                 "curr #free segs %d, new seg offset %d, occupied size %d, "
                 "%d items",
            new_seg->ttl, n_merged, at_most_n_seg, merged_segs, new_seg_id,
            heap.n_free_seg, new_seg->write_offset,
            new_seg->occupied_size, new_seg->n_item);
        successful_merge += 1;
    }

    log_verb("***************************************************");
    INCR_N(seg_metrics, seg_merge, n_merged);

    return heap.segs[new_seg_id].next_seg_id;
}



