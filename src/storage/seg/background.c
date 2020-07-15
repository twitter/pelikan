
#include "background.h"
#include "item.h"
#include "seg.h"
#include "segevict.h"
#include "ttlbucket.h"

#include "cc_debug.h"
#include "time/cc_wheel.h"

#include <pthread.h>
#include <sysexits.h>
#include <time.h>

extern volatile bool stop;
extern volatile proc_time_i flush_at;
extern pthread_t bg_tid;


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

            /* ttl_bucket first_seg can change */
//            ASSERT(next_seg_id == ttl_buckets[i].first_seg_id);
            seg_id = next_seg_id;
            seg = &heap.segs[seg_id];
        }
    }
}

static void
_merge_seg(void)
{
    ;
}


static void *
_background_loop(void *data)
{
    log_info("seg background thread started");

    proc_time_fine_i curr_msec;
    while (!stop) {
        curr_msec = time_proc_ms();
        _check_seg_expire();
        _merge_seg();
//        if (time_proc_ms() - curr_msec < 400)
//            usleep(100000);
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

