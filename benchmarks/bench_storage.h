#pragma once

#include <pthread.h>
#include <time/time.h>

#define MAX_KEY_LEN 255
#define MAX_VAL_LEN 8 * 1024 * 1024
#define ENTRY_SIZE sizeof(struct benchmark_entry)

// typedef size_t benchmark_key_u;

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

static const char *op_names[op_invalid + 1] = {"get", "gets", "set", "add",
        "cas", "replace", "append", "prepend", "delete", "incr", "decr",
        "cache_miss", "invalid"};


struct benchmark_entry {
    char *key;
    char *val;
    uint32_t key_len : 8;
    uint32_t val_len : 24;
    uint64_t delta;
    proc_time_i expire_at;
    op_e op;
};

struct benchmark {
    struct benchmark_entry *entries;
    void *options;
    void *warmup_reader; /*used in trace_replay */
    void *eval_reader; /* used in trace_replay */
    uint64_t n_warmup_req; /* used in trace_replay, and no warmup reader is
                              supplied */
    delta_time_i default_ttl;
    int64_t op_cnt[op_invalid];

    struct operation_latency {
        struct duration *samples;
        op_e *ops; /* can change to uint8_t* to reduce memory footprint */
        size_t count;
    } latency;
    uint8_t n_thread;
    uint64_t n_req;
    uint64_t n_miss;
};


#define BENCH_OPTS(b) ((struct benchmark_options *)((b)->options))
#define O(b, opt) option_uint(&(BENCH_OPTS(b)->benchmark.opt))
#define O_UINT(b, opt) option_uint(&(BENCH_OPTS(b)->benchmark.opt))
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


/* this is not placed in shared.c because in throughput mode, we want to avoid
 * the op count and latency measurement
 */
static inline rstatus_i
run_op(struct benchmark_entry *e)
{
    log_verb("** start a new request ts % " PRId32 " key %.*s, op %s, ttl "
                                                   "%" PRId32,
            proc_sec, e->key_len, e->key, op_names[e->op],
            e->expire_at - proc_sec);

    switch (e->op) {
    case op_get:
        return bench_storage_get(e);
    case op_set:
        return bench_storage_set(e);
    case op_gets:
        return bench_storage_gets(e);
    case op_cas:
        return bench_storage_cas(e);
    case op_add:
        return bench_storage_add(e);
    case op_replace:
        return bench_storage_replace(e);
    case op_delete:
        return bench_storage_delete(e);
    case op_incr:
        return bench_storage_incr(e);
    case op_decr:
        return bench_storage_decr(e);
    default:
        break;
        log_crit("op %s not implemented", op_names[e->op]);
        NOT_REACHED();
    }

    NOT_REACHED();
    return CC_ERROR;
}