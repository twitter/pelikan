#include <bench_storage.h>

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_mm.h>
#include <time/cc_timer.h>

#include <errno.h>
#include <pthread.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>


static __thread unsigned int rseed = 1234; /* XXX: make this an option */

#define RRAND(min, max) (rand_r(&(rseed)) % ((max) - (min) + 1) + (min))

#define SWAP(a, b)                                                             \
    do {                                                                       \
        __typeof__(a) _tmp = (a);                                              \
        (a) = (b);                                                             \
        (b) = _tmp;                                                            \
    } while (0)

#define BENCHMARK_OPTION(ACTION)                                               \
    ACTION(entry_min_size, OPTION_TYPE_UINT, 64, "Min size of cache entry")    \
    ACTION(entry_max_size, OPTION_TYPE_UINT, 64, "Max size of cache entry")    \
    ACTION(nentries, OPTION_TYPE_UINT, 1000,                                   \
            "Max total number of cache entries")                               \
    ACTION(nops, OPTION_TYPE_UINT, 100000, "Total number of operations")       \
    ACTION(pct_get, OPTION_TYPE_UINT, 80, "% of gets")                         \
    ACTION(pct_put, OPTION_TYPE_UINT, 10, "% of puts")                         \
    ACTION(pct_rem, OPTION_TYPE_UINT, 10, "% of removes")                      \
    ACTION(per_op_latency, OPTION_TYPE_BOOL, true, "Collect latency samples")  \
    ACTION(debug_logging, OPTION_TYPE_BOOL, false, "turn on debug logging")

struct benchmark_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct benchmark_specific benchmark;
    debug_options_st debug;
    struct option engine[]; /* storage-engine specific options... */
};


static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    b->entries = NULL;

    unsigned nopts = OPTION_CARDINALITY(struct benchmark_specific);

    struct benchmark_specific opts = {BENCHMARK_OPTION(OPTION_INIT)};
    option_load_default((struct option *)&opts, nopts);

    debug_options_st debug_opts = {DEBUG_OPTION(OPTION_INIT)};
    option_load_default(
            (struct option *)&debug_opts, OPTION_CARDINALITY(debug_options_st));

    nopts += bench_storage_config_nopts();
    nopts += OPTION_CARDINALITY(debug_options_st);

    b->options = cc_alloc(sizeof(struct option) * nopts);
    ASSERT(b->options != NULL);
    ((struct benchmark_options *)(b->options))->benchmark = opts;
    ((struct benchmark_options *)(b->options))->debug = debug_opts;

    bench_storage_config_init(
            ((struct benchmark_options *)(b->options))->engine);


    if (config != NULL) {
        FILE *fp = fopen(config, "r");
        if (fp == NULL) {
            log_crit("failed to open the config file");
            cc_free(b->options);

            return CC_EINVAL;
        }
        option_load_file(fp, (struct option *)b->options, nopts);
        fclose(fp);
    }

    if (O_BOOL(b, debug_logging)) {
        if (debug_setup(&((struct benchmark_options *)(b->options))->debug) !=
                CC_OK) {
            log_stderr("debug log setup failed");
            exit(EX_CONFIG);
        }
    }

    if (O(b, entry_min_size) <= sizeof(uint32_t)) {
        log_crit(
                "entry_min_size must larger than %lu", sizeof(uint32_t));
        cc_free(b->options);

        return CC_EINVAL;
    }

    if (O_BOOL(b, per_op_latency)){
        b->latency.samples = cc_alloc(O(b, nops) * sizeof(struct duration));
        ASSERT(b->latency.samples != NULL);
    }

    b->latency.ops = cc_alloc(O(b, nops) * sizeof(op_e));
    ASSERT(b->latency.ops != NULL);

    b->latency.count = 0;

    return CC_OK;
}

static void
benchmark_destroy(struct benchmark *b)
{
    cc_free(b->latency.samples);
    cc_free(b->latency.ops);

    cc_free(b->options);
}

static struct benchmark_entry
benchmark_entry_create(uint32_t key, size_t size)
{
    struct benchmark_entry e;
    e.key_len = 16;      /* this needs to be large enough */
    e.val_len = size - sizeof(key);
    e.key = cc_alloc(e.key_len);
    ASSERT(e.key != NULL);
    e.val = cc_alloc(e.val_len);
    ASSERT(e.val != NULL);

    int ret = snprintf(e.key, e.key_len, "%"PRIu32, key);
    ASSERT(ret > 0);

    memset(e.val, 'a', e.val_len);
    e.val[e.val_len - 1] = 0;

    e.expire_at = INT32_MAX;

    return e;
}

static void
benchmark_entry_destroy(struct benchmark_entry *e)
{
    cc_free(e->key);
    cc_free(e->val);
}

static void
benchmark_entries_populate(struct benchmark *b)
{
    size_t nentries = O(b, nentries);
    b->entries = cc_alloc(sizeof(struct benchmark_entry) * nentries);
    ASSERT(b->entries != NULL);

    for (size_t i = 1; i <= nentries; ++i) {
        size_t size = RRAND(O(b, entry_min_size), O(b, entry_max_size));
        b->entries[i - 1] = benchmark_entry_create(i, size);
    }
}

static void
benchmark_entries_delete(struct benchmark *b)
{
    for (size_t i = 0; i < O(b, nentries); ++i) {
        benchmark_entry_destroy(&b->entries[i]);
    }
    cc_free(b->entries);
}


static struct duration
benchmark_run(struct benchmark *b)
{
    bool per_op_latency = O_BOOL(b, per_op_latency);

    struct array *in;
    struct array *in2;
    struct array *out;

    size_t nentries = O(b, nentries);

    bench_storage_init(((struct benchmark_options *)(b->options))->engine,
            O(b, entry_max_size), nentries);

    array_create(&in, nentries, sizeof(struct benchmark_entry *));
    array_create(&in2, nentries, sizeof(struct benchmark_entry *));
    array_create(&out, nentries, sizeof(struct benchmark_entry *));

    for (size_t i = 0; i < nentries; ++i) {
        struct benchmark_entry **e = array_push(in);
        *e = &b->entries[i];

        ASSERT(bench_storage_set(*e) == CC_OK);
    }

    struct duration d;
    duration_start(&d);

    for (size_t i = 0; i < O(b, nops); ++i) {
        if (array_nelem(in) == 0) {
            SWAP(in, in2);
            /* XXX: array_shuffle(in) */
        }

        unsigned pct = RRAND(0, 100);

        unsigned pct_sum = 0;

        if (pct_sum <= pct && pct < O(b, pct_get) + pct_sum) {
            ASSERT(array_nelem(in) != 0);
            struct benchmark_entry **e = array_pop(in);
            (*e)->op = op_get;
            log_verb("benchmark get(%.*s)", (*e)->key_len, (*e)->key);

            if (benchmark_run_operation(b, *e, per_op_latency) != CC_OK) {
                log_info("benchmark get(%.*s) failed", (*e)->key_len, (*e)->key);
            }

            struct benchmark_entry **e2 = array_push(in2);
            *e2 = *e;
        }
        pct_sum += O(b, pct_get);
        if (pct_sum <= pct && pct < O(b, pct_put) + pct_sum) {
            struct benchmark_entry **e;
            if (array_nelem(out) != 0) {
                e = array_pop(out);
            } else {
                ASSERT(array_nelem(in) != 0);
                e = array_pop(in);
                (*e)->op = op_delete;

                log_verb("benchmark rem(%.*s) for set", (*e)->key_len, (*e)->key);
                if (bench_storage_delete(*e) != CC_OK) {
                    log_info("benchmark rem(%.*s) for set failed", (*e)->key_len, (*e)->key);
                }
            }

            (*e)->op = op_set;
            log_verb("benchmark set(%.*s)", (*e)->key_len, (*e)->key);
            if (benchmark_run_operation(b, *e, per_op_latency) != CC_OK) {
                log_info("benchmark put(%.*s) failed", (*e)->key_len, (*e)->key);
            }

            struct benchmark_entry **e2 = array_push(in2);
            *e2 = *e;
        }
        pct_sum += O(b, pct_put);
        if (pct_sum < pct && pct <= O(b, pct_rem) + pct_sum) {
            ASSERT(array_nelem(in) != 0);
            struct benchmark_entry **e = array_pop(in);
            (*e)->op = op_delete;

            log_verb("benchmark rem(%.*s)", (*e)->key_len, (*e)->key);
            if (benchmark_run_operation(b, *e, per_op_latency) != CC_OK) {
                log_info("benchmark rem(%.*s) failed", (*e)->key_len, (*e)->key);
            }

            struct benchmark_entry **e2 = array_push(out);
            *e2 = *e;
        }
    }

    duration_stop(&d);

    bench_storage_deinit();

    array_destroy(&in);
    array_destroy(&in2);
    array_destroy(&out);

    return d;
}


int
main(int argc, char *argv[])
{
    struct benchmark b;
    if (benchmark_create(&b, argv[1]) != 0) {
        loga("failed to create benchmark instance");
        return -1;
    }

    benchmark_entries_populate(&b);

    struct duration d = benchmark_run(&b);

    benchmark_print_summary(&b, &d, O_BOOL(&b, per_op_latency));

    benchmark_entries_delete(&b);

    benchmark_destroy(&b);

    return 0;
}
