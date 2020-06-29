#pragma once

#include <cc_define.h>
#include <cc_mm.h>
#include <time/cc_timer.h>
#include <time/time.h>

#include <stddef.h>

typedef size_t benchmark_key_u;

typedef enum {
    op_get = 0,
    op_gets,
    op_set,
    op_add,
    op_cas,
    op_replace,
    op_append,
    op_prepend,
    op_delete,
    op_incr,
    op_decr,

    op_invalid
} op_e;

static const char *op_names[op_invalid] = {"get", "gets", "set", "add", "cas",
        "replace", "append", "prepend", "delete", "incr", "decr"};

struct benchmark_entry {
    char *key;
    benchmark_key_u key_len;
    char *val;
    size_t val_len;
    op_e op;
    delta_time_i ttl;
};

struct benchmark {
    struct benchmark_entry *entries;
    void *options;
    void *reader; /* used in trace_replay */

    struct operation_latency {
        struct duration *samples;
        op_e *ops;
        size_t count;
    } latency;
};


#define BENCH_OPTS(b) ((struct benchmark_options *)((b)->options))
#define O(b, opt) option_uint(&(BENCH_OPTS(b)->benchmark.opt))
#define O_BOOL(b, opt) option_bool(&(BENCH_OPTS(b)->benchmark.opt))
#define O_STR(b, opt) option_str(&(BENCH_OPTS(b)->benchmark.opt))

rstatus_i
bench_storage_init(void *opts, size_t item_size, size_t nentries);
rstatus_i
bench_storage_deinit(void);
rstatus_i
bench_storage_put(struct benchmark_entry *e);
rstatus_i
bench_storage_get(struct benchmark_entry *e);
rstatus_i
bench_storage_rem(struct benchmark_entry *e);
unsigned
bench_storage_config_nopts(void);
void
bench_storage_config_init(void *opts);


/* shared functions */
static inline void
benchmark_print_summary(
        struct benchmark *b, struct duration *d, bool per_op_latency)
{
    printf("total benchmark runtime: %f s, throughput %.2f M QPS\n",
            duration_sec(d), b->latency.count / duration_sec(d) / 1000000);
    printf("average operation latency: %f ns\n",
            duration_ns(d) / b->latency.count);

    if (!per_op_latency){
        return;
    }

    struct operation_latency *latency = &b->latency;
    struct duration *all_durations =
            cc_zalloc(sizeof(struct duration) * latency->count);
    size_t pos = 0;

    for (op_e op = 0; op < op_invalid; ++op) {
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

static inline rstatus_i
benchmark_run_operation(
        struct benchmark *b, struct benchmark_entry *e, bool per_op_latency)
{
    rstatus_i status = CC_OK;

    struct operation_latency *latency = &b->latency;
    size_t nsample = latency->count++;

    if (per_op_latency) {
        duration_start_type(&latency->samples[nsample], DURATION_FAST);
    }

    switch (e->op) {
    case op_get:
        status = bench_storage_get(e);
        break;
    case op_set:
        status = bench_storage_put(e);
        break;
    case op_delete:
        status = bench_storage_rem(e);
        break;
    default:
        break;
        log_crit("op %s not implemented", op_names[e->op]);
        NOT_REACHED();
    }

    if (per_op_latency) {
        duration_stop(&latency->samples[nsample]);
        latency->ops[nsample] = e->op;
    }

    ASSERT(status == CC_OK);
    return status;
}
