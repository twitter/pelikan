
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
extern struct ttl_bucket        ttl_buckets[MAX_N_TTL_BUCKET];


static void
_check_seg_expire(void)
{
    int i;
    rstatus_i status;
    proc_time_i curr_sec = time_proc_sec();
    struct seg *seg;
    int32_t seg_id, next_seg_id;
    for (i = 0; i < MAX_N_TTL_BUCKET; i++) {
        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id == -1) {
            continue;
        }
        seg = &heap.segs[seg_id];
        /* curr_sec - 2 is to reduce data race when the expiring segment
         * is being written to */
        while (seg->create_at + seg->ttl < curr_sec - 2 ||
                seg->create_at < flush_at) {
            log_debug("curr_sec %" PRId32 ": expire seg %" PRId32 " create "
                      "at %" PRId32 " ttl %" PRId32 " flushed at %" PRId32,
                    curr_sec, seg_id, seg->create_at, seg->ttl, flush_at);
            next_seg_id = seg->next_seg_id;

            status = seg_rm_expired_seg(seg_id);
            if (status != CC_OK) {
                log_error("error removing expired seg %d", seg_id);
            }

            if (next_seg_id == -1)
                break;

            seg_id = next_seg_id;
            seg = &heap.segs[seg_id];
        }
    }
}

static inline void
_check_merge_seg(void)
{
#define N_SEG_HIGH_WM 8
#define N_SEG_LOW_WM 2


    struct seg *seg;
    int32_t seg_id;

    for (int i = 0; i < MAX_N_TTL_BUCKET; i++) {
        if (heap.n_free_seg > N_SEG_HIGH_WM) {
            return;
        }

        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id == -1) {
            continue;
        }
        seg = &heap.segs[seg_id];
        /* TODO (jason): change it to be more efficient - avoid double comp */
        while (!(seg_mergeable(seg->seg_id) &&
                seg_mergeable(seg->next_seg_id))) {
            if (seg->next_seg_id == -1)
                break;
            seg = &heap.segs[seg->next_seg_id];
        }

        /* either found a mergeable seg or reach end of seg list */
        if (seg->next_seg_id == -1) {
            continue;
        }
        merge_segs(seg->seg_id, -1);
    }


    static proc_time_i last_merge = 0, last_clear = 0;
    if (last_merge == 0) {
        last_merge = time_proc_sec();
        last_clear = time_proc_sec();
    }

#ifdef TRACK_ADVANCED_STAT
    if (time_proc_sec() - last_clear > 1200) {
        last_clear = time_proc_sec();
        for (int32_t i = 0; i < heap.max_nseg; i++) {
            heap.segs[i].n_active = 0;
            memset(heap.segs[i].active_obj, 0, 131072 * sizeof(bool));
        }
    }
#endif
}

static inline void
_check_merge_two_seg(void){

    int32_t seg_id, next_seg_id;
    struct seg *seg, *next_seg;

    for (seg_id = 0; seg_id < heap.max_nseg; seg_id++) {
        seg = &heap.segs[seg_id];
        if (!seg_mergeable(seg->seg_id))
            continue;
        next_seg_id = seg->next_seg_id;
        next_seg = &heap.segs[next_seg_id];
        if (!seg_mergeable(next_seg_id))
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
        if (duration_ms(&d) < 20){
            usleep(20000);
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

