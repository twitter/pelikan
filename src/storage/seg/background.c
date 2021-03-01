
#include "background.h"
#include "item.h"
#include "seg.h"
#include "ttlbucket.h"

#include "cc_debug.h"
#include "time/cc_wheel.h"

#include <pthread.h>
#include <sysexits.h>
#include <time.h>

extern volatile bool        stop;
extern volatile proc_time_i flush_at;
extern pthread_t            bg_tid;
extern struct ttl_bucket    ttl_buckets[MAX_N_TTL_BUCKET];


static void
check_seg_expire(void)
{
    rstatus_i   status;
    struct seg  *seg;
    int32_t     seg_id, next_seg_id;
    for (int i = 0; i < MAX_N_TTL_BUCKET; i++) {
        seg_id = ttl_buckets[i].first_seg_id;
        if (seg_id == -1) {
            /* no object of this TTL */
            continue;
        }

        seg = &heap.segs[seg_id];
        /* curr_sec - 2 to avoid a slow client is still writing to
         * the expiring segment  */
        while (seg->create_at + seg->ttl < time_proc_sec() - 2 ||
            seg->create_at < flush_at) {
            log_debug("expire seg %"PRId32 ", create at %"PRId32 ", ttl %"PRId32
            ", flushed at %"PRId32, seg_id, seg->create_at, seg->ttl, flush_at);

            next_seg_id = seg->next_seg_id;

            status = expire_seg(seg_id);
            if (status != CC_OK) {
                log_error("error removing expired seg %d", seg_id);
            }

            if (next_seg_id == -1) {
                break;
            }

            seg_id = next_seg_id;
            seg    = &heap.segs[seg_id];
        }
    }
}

static void *
background_main(void *data)
{

//    pthread_setname_np(pthread_self(), "segBg");

    log_info("Segcache background thread started");

    while (!stop) {
        check_seg_expire();

        // do we want to enable background eviction?
        // merge_based_eviction();

        usleep(200000);
    }

    log_info("seg background thread stopped");
    return NULL;
}

void
start_background_thread(void *arg)
{
    int ret = pthread_create(&bg_tid, NULL, background_main, arg);
    if (ret != 0) {
        log_crit("pthread create failed for background thread: %s",
            strerror(ret));
        exit(EX_OSERR);
    }
}

