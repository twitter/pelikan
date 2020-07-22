
#include "background.h"
#include "item.h"
#include "seg.h"
#include "ttlbucket.h"

#include "cc_debug.h"
#include "time/cc_wheel.h"

#include <pthread.h>
#include <sysexits.h>
#include <time.h>

#define CHECK_MERGE_INTVL       20

extern volatile bool            stop;
extern volatile proc_time_i     flush_at;
extern pthread_t                bg_tid;
extern struct ttl_bucket        ttl_buckets[MAX_TTL_BUCKET];

/* used for tracking mergeable segments */
extern int32_t                  mergable_seg[MAX_N_MERGEABLE_SEG];
extern int32_t                  n_mergeable_seg;
extern pthread_mutex_t          misc_lock;



static void
_check_seg_expire(void)
{
    int i;
    proc_time_i curr_sec = time_proc_sec();
    struct seg *seg;
    int32_t seg_id, next_seg_id;
    for (i = 0; i < MAX_TTL_BUCKET; i++) {
        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id == -1) {
            continue;
        }
        seg = &heap.segs[seg_id];
        while (seg->create_at + seg->ttl < curr_sec ||
                seg->create_at < flush_at) {
            log_debug("curr_sec %" PRId32 ": expire seg %" PRId32 " create "
                      "at %" PRId32 " ttl %" PRId32 " flushed at %" PRId32,
                    curr_sec, seg_id, seg->create_at, seg->ttl, flush_at);
            next_seg_id = seg->next_seg_id;

            seg_rm_expired_seg(seg_id);

            if (next_seg_id == -1)
                break;

            seg_id = next_seg_id;
            seg = &heap.segs[seg_id];
        }
    }
}

static inline bool _seg_is_mergeable(struct seg *seg) {
    bool is_mergeable;
    is_mergeable = seg->occupied_size <= heap.seg_size * SEG_MERGE_THRESHOLD;
    is_mergeable = is_mergeable && seg->evictable == 1;
//    is_mergeable = is_mergeable && seg->next_seg_id != -1;
    /* a magic number - we don't want to merge just created seg */
    is_mergeable = is_mergeable && time_proc_sec() - seg->create_at > 60;
    /* don't merge segments that will expire */
    is_mergeable = is_mergeable &&
            seg->create_at + seg->ttl - time_proc_sec() > 60;
    return is_mergeable;
}


static inline void
_check_merge_seg(void)
{
    static proc_time_i last_check = 0;

    if (time_proc_sec() - last_check < CHECK_MERGE_INTVL)
        return;

    last_check = time_proc_sec();

    struct seg *seg, *next_seg;
    int32_t seg_id, next_seg_id;
    for (seg_id = 0; seg_id < heap.max_nseg; seg_id++) {
        seg = &heap.segs[seg_id];
        if (!_seg_is_mergeable(seg))
            continue;
        next_seg_id = seg->next_seg_id;
        next_seg = &heap.segs[next_seg_id];
        if (!_seg_is_mergeable(next_seg))
            continue;

        merge_seg(seg_id, next_seg_id);
    }
}


static void *
_background_loop(void *data)
{
    log_info("seg background thread started");

    struct duration d;
    while (!stop) {
        duration_start(&d);

        _check_seg_expire();
//        _check_merge_seg();

        duration_stop(&d);
        if (duration_ms(&d) < 400){
            usleep(100000);
            /* TODO(jason): add a metric here */
        }
    }
    log_info("seg background thread stopped");
    return NULL;
}


void
start_background_thread(void *arg)
{
    int ret = pthread_create(&bg_tid, NULL, _background_loop, arg);
    if (ret != 0) {
        log_crit("pthread create failed for background thread: %s",
                strerror(ret));
        exit(EX_OSERR);
    }
}

