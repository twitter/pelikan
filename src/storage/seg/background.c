
#include "seg.h"
#include "segevict.h"
#include "ttlbucket.h"
#include "item.h"
#include "background.h"

#include "time/cc_wheel.h"
#include "cc_debug.h"

#include <time.h>
#include <pthread.h>
#include <sysexits.h>

extern bool stop;
extern proc_time_i flush_at;
//static struct timing_wheel *tw;


static void _check_seg_expire(){
    int i;
    proc_time_i curr_sec = time_proc_sec();
    struct seg *seg;
    int32_t seg_id, next_seg_id;
    for (i=0; i<MAX_TTL_BUCKET; i++) {
        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id == -1) {
            continue;
        }
        seg = &heap.segs[seg_id];
        while (seg->create_at + seg->ttl < curr_sec || seg->create_at < flush_at) {
            log_debug("expire seg %" PRId32 "create at %"PRId32 " + ttl %"PRId32 " < %"PRId32,
                seg_id, seg->create_at, seg->ttl, curr_sec);
            next_seg_id = seg->next_seg_id;

            seg_rm_expired_seg(seg_id);

            if (next_seg_id == -1)
                break;

            ASSERT(next_seg_id == ttl_buckets[i].first_seg_id);
            seg_id = next_seg_id;
            seg = &heap.segs[seg_id];
        }
    }
}

static void _merge_seg() {
    ;
}




static void *_background_loop(void *data) {
    log_info("seg background thread started");

    proc_time_fine_i curr_msec;
    while (!stop) {
//        timing_wheel_execute(tw);
        curr_msec = time_proc_ms();
        _check_seg_expire();
        _merge_seg();
        if (time_proc_ms() - curr_msec < 400)
            usleep(100000);
    }
    return NULL;
}





void start_background_thread(void *arg){
//    struct timeout tick;
//    timeout_set_ms(&tick, 10);
//    tw = timing_wheel_create();

    pthread_t  bg_pid;

    int ret = pthread_create(&bg_pid, NULL, _background_loop, arg);
    if (ret != 0) {
        log_crit("pthread create failed for background thread: %s", strerror(ret));
        exit(EX_OSERR);
    }



}
