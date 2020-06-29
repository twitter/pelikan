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
    ACTION(latency, OPTION_TYPE_BOOL, true, "Collect latency samples")

#define O(b, opt) option_uint(&(b->options->benchmark.opt))
#define O_BOOL(b, opt) option_bool(&(b->options->benchmark.opt))


struct benchmark_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct benchmark_specific benchmark;
    debug_options_st debug;
    struct option engine[]; /* storage-engine specific options... */
};

struct benchmark {
    struct benchmark_entry *entries;
    struct benchmark_options *options;

    struct operation_latency {
        struct duration *samples;
        op_e *ops;
        size_t count;
    } latency;
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
    b->options->benchmark = opts;
    b->options->debug = debug_opts;

    bench_storage_config_init(b->options->engine);


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

    if (O(b, entry_min_size) <= sizeof(benchmark_key_u)) {
        log_crit(
                "entry_min_size must larger than %lu", sizeof(benchmark_key_u));
        cc_free(b->options);

        return CC_EINVAL;
    }

    b->latency.samples = O_BOOL(b, latency) ?
            cc_alloc(O(b, nops) * sizeof(struct duration)) :
            NULL;
    b->latency.ops =
            O_BOOL(b, latency) ? cc_alloc(O(b, nops) * sizeof(op_e)) : NULL;
    b->latency.count = 0;

    if (debug_setup(&b->options->debug) != CC_OK) {
        log_stderr("debug log setup failed");
        exit(EX_CONFIG);
    }

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
benchmark_entry_create(benchmark_key_u key, size_t size)
{
    struct benchmark_entry e;
    e.key_len = sizeof(key);
    e.val_len = size - sizeof(key);
    e.key = cc_alloc(e.key_len);
    ASSERT(e.key != NULL);
    e.val = cc_alloc(e.val_len);
    ASSERT(e.val != NULL);

    int ret = snprintf(e.key, e.key_len, "%zu", key);
    ASSERT(ret > 0);

    memset(e.val, 'a', e.val_len);
    e.val[e.val_len - 1] = 0;

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

static void
benchmark_print_summary(struct benchmark *b, struct duration *d)
{
    printf("total benchmark runtime: %f s\n", duration_sec(d));
    printf("average operation latency: %f ns\n", duration_ns(d) / O(b, nops));
    if (!O_BOOL(b, latency))
        return;

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
        struct duration *p50 =
                &latency->samples[(size_t)(latency->count * 0.5)];
        struct duration *p99 =
                &latency->samples[(size_t)(latency->count * 0.99)];
        struct duration *p999 =
                &latency->samples[(size_t)(latency->count * 0.999)];
        printf("Latency p50, p99, p99.9 for %s (%lu samples): %f, %f, %f\n",
                op_names[op], pos, duration_ns(p50),
                duration_ns(p99), duration_ns(p999));
    }
}

static rstatus_i
benchmark_run_operation(struct benchmark *b, struct benchmark_entry *e, op_e op)
{
    rstatus_i status = CC_OK;

    struct operation_latency *latency = &b->latency;
    size_t nsample = latency->count++;

    if (O_BOOL(b, latency))
        duration_start_type(&latency->samples[nsample], DURATION_FAST);

    switch (op) {
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
        log_crit("op %s not implemented", op_names[op]);
        NOT_REACHED();
    }

    if (O_BOOL(b, latency)){
        duration_stop(&latency->samples[nsample]);
        latency->ops[nsample] = op;
    }

    ASSERT(status == CC_OK);
    return status;
}

static struct duration
benchmark_run(struct benchmark *b)
{
    struct array *in;
    struct array *in2;
    struct array *out;

    size_t nentries = O(b, nentries);

    bench_storage_init(b->options->engine, O(b, entry_max_size), nentries);

    array_create(&in, nentries, sizeof(struct benchmark_entry *));
    array_create(&in2, nentries, sizeof(struct benchmark_entry *));
    array_create(&out, nentries, sizeof(struct benchmark_entry *));

    for (size_t i = 0; i < nentries; ++i) {
        struct benchmark_entry **e = array_push(in);
        *e = &b->entries[i];

        ASSERT(bench_storage_put(*e) == CC_OK);
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

            if (benchmark_run_operation(b, *e, op_get) != CC_OK) {
                log_info("benchmark get() failed");
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
                if (bench_storage_rem(*e) != CC_OK) {
                    log_info("benchmark rem() failed");
                }
            }

            if (benchmark_run_operation(b, *e, op_set) != CC_OK) {
                log_info("benchmark put() failed");
            }

            struct benchmark_entry **e2 = array_push(in2);
            *e2 = *e;
        }
        pct_sum += O(b, pct_put);
        if (pct_sum < pct && pct <= O(b, pct_rem) + pct_sum) {
            ASSERT(array_nelem(in) != 0);
            struct benchmark_entry **e = array_pop(in);

            if (benchmark_run_operation(b, *e, op_delete) != CC_OK) {
                log_info("benchmark rem() failed");
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

    benchmark_print_summary(&b, &d);

    benchmark_entries_delete(&b);

    benchmark_destroy(&b);

    return 0;
}
