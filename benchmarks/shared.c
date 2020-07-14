
#include "bench_storage.h"

#include <cc_define.h>
#include <cc_mm.h>
#include <time/cc_timer.h>
#include <time/time.h>

#include <stddef.h>


/* shared functions */
void
benchmark_print_summary(
        struct benchmark *b, struct duration *d, bool per_op_latency)
{
    printf("total benchmark runtime: %f s, throughput %.2f M QPS\n",
            duration_sec(d), b->latency.count / duration_sec(d) / 1000000);
    printf("average operation latency: %f ns\n",
            duration_ns(d) / b->latency.count);

    struct operation_latency *latency = &b->latency;

    if (!per_op_latency) {
        for (int i = 0; i < op_invalid; i++) {
            if (b->op_cnt[i] == 0)
                continue;
            printf("%16s %16" PRId64 "\t (%8.2lf%%)\n", op_names[i], b->op_cnt[i],
                    (double)b->op_cnt[i] / latency->count * 100);
        }
        return;
    }

    size_t pos = 0;
    struct duration *all_durations =
            cc_zalloc(sizeof(struct duration) * latency->count);

    for (op_e op = 0; op <= op_invalid; ++op) {
        pos = 0;
        /* collect all samples of this op */
        for (int i = 0; i < latency->count; i++) {
            if (latency->ops[i] == op) {
                all_durations[pos++] = latency->samples[i];
            }
        }

        if (pos == 0) {
            continue;
        }

        qsort(all_durations, pos, sizeof(struct duration), duration_compare);
        struct duration *p50 = &all_durations[(size_t)(pos * 0.5)];
        struct duration *p99 = &all_durations[(size_t)(pos * 0.99)];
        struct duration *p999 = &all_durations[(size_t)(pos * 0.999)];
        printf("Latency p50, p99, p99.9 for %s (%lu samples): %f, %f, %f\n",
                op_names[op], pos, duration_ns(p50), duration_ns(p99),
                duration_ns(p999));
    }
}


rstatus_i
benchmark_run_operation(
        struct benchmark *b, struct benchmark_entry *e, bool per_op_latency)
{
    /* TODO(jason): for accuracy, move latency to thread local variable */
    rstatus_i status = CC_OK;
    size_t nsample;
    struct operation_latency *latency;

    log_verb("** start a new request key %.*s, op %s, ttl %" PRId32, e->key_len,
            e->key, op_names[e->op], e->expire_at - proc_sec);

    latency = &b->latency;
    nsample = __atomic_fetch_add(&latency->count, 1, __ATOMIC_RELAXED);

    if (per_op_latency) {
        duration_start_type(&latency->samples[nsample], DURATION_FAST);
    }

    status = run_op(e);

    op_e op = status == CC_OK ? e->op : op_failed;

    if (per_op_latency) {
        duration_stop(&latency->samples[nsample]);
        latency->ops[nsample] = op;
    }

    __atomic_fetch_add(&b->op_cnt[op], 1, __ATOMIC_RELAXED);

    return status;
}

