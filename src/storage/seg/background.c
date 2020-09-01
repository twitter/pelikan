
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

