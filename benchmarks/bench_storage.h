#pragma once

#include <time/time.h>

#define MAX_KEY_LEN 255
#define ENTRY_SIZE sizeof(struct benchmark_entry)

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
    op_failed,

    op_invalid
} op_e;

struct benchmark_entry {
    char *key;
    benchmark_key_u key_len;
    char *val;
    size_t val_len;
    op_e op;
    proc_time_i expire_at;
    uint64_t delta;
};

struct benchmark {
    struct benchmark_entry *entries;
    void *options;
    void *reader; /* used in trace_replay */
    int64_t op_cnt[op_invalid];

    struct operation_latency {
        struct duration *samples;
        op_e *ops;      /* can change to uint8_t* to reduce memory footprint */
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
bench_storage_get(struct benchmark_entry *e);
rstatus_i
bench_storage_gets(struct benchmark_entry *e);
rstatus_i
bench_storage_delete(struct benchmark_entry *e);

rstatus_i
bench_storage_set(struct benchmark_entry *e);
rstatus_i
bench_storage_cas(struct benchmark_entry *e);
rstatus_i
bench_storage_add(struct benchmark_entry *e);
rstatus_i
bench_storage_replace(struct benchmark_entry *e);


rstatus_i
bench_storage_incr(struct benchmark_entry *e);
rstatus_i
bench_storage_decr(struct benchmark_entry *e);

unsigned
bench_storage_config_nopts(void);
void
bench_storage_config_init(void *opts);


void
benchmark_print_summary(
        struct benchmark *b, struct duration *d, bool per_op_latency);

rstatus_i
benchmark_run_operation(
        struct benchmark *b, struct benchmark_entry *e, bool per_op_latency);
