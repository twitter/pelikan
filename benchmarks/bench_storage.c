#include <stdio.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <pthread.h>

#include <bench_storage.h>
#include <time/cc_timer.h>
#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_array.h>

static __thread unsigned int rseed = 1234; /* XXX: make this an option */

#define RRAND(min, max) (rand_r(&(rseed)) % ((max) - (min) + 1) + (min))

#define SWAP(a, b) do {\
    __typeof__(a) _tmp = (a);\
    (a) = (b);\
    (b) = _tmp;\
} while (0)

#define BENCHMARK_OPTION(ACTION)\
    ACTION(entry_min_size,  OPTION_TYPE_UINT, 64,    "Min size of cache entry")\
    ACTION(entry_max_size,  OPTION_TYPE_UINT, 64,    "Max size of cache entry")\
    ACTION(nentries,        OPTION_TYPE_UINT, 1000,  "Max total number of cache entries" )\
    ACTION(nops,            OPTION_TYPE_UINT, 100000,"Total number of operations")\
    ACTION(pct_get,         OPTION_TYPE_UINT, 80,    "% of gets")\
    ACTION(pct_put,         OPTION_TYPE_UINT, 10,    "% of puts")\
    ACTION(pct_rem,         OPTION_TYPE_UINT, 10,    "% of removes")

#define O(b, opt) option_uint(&(b->options.benchmark.opt))

struct benchmark_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct benchmark_specific benchmark;
};

struct benchmark {
    FILE *config;
    struct benchmark_entry *entries;
    struct benchmark_options options;
};

static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    b->entries = NULL;

    struct benchmark_specific opts1 = { BENCHMARK_OPTION(OPTION_INIT) };
    b->options.benchmark = opts1;

    size_t nopts = OPTION_CARDINALITY(struct benchmark_options);
    option_load_default((struct option *)&b->options, nopts);

    if (config != NULL) {
        b->config = fopen(config, "r");
        if (b->config == NULL) {
            log_crit("failed to open the config file");
            return CC_EINVAL;
        }
        option_load_file(b->config, (struct option *)&b->options, nopts);
        fclose(b->config);
    }

    if (O(b, entry_min_size) <= sizeof(benchmark_key_u)) {
        log_crit("entry_min_size must larger than %lu",
            sizeof(benchmark_key_u));

        return CC_EINVAL;
    }

    return CC_OK;
}

static void
benchmark_destroy(struct benchmark *b)
{
}

static struct benchmark_entry
benchmark_entry_create(benchmark_key_u key, size_t size)
{
    struct benchmark_entry e;
    e.key_size = sizeof(key);
    e.value_size = size - sizeof(key);
    e.key = cc_alloc(e.key_size);
    ASSERT(e.key != NULL);
    e.value = cc_alloc(e.value_size);
    ASSERT(e.value != NULL);

    int ret = snprintf(e.key, e.key_size, "%zu", key);
    ASSERT(ret > 0);

    memset(e.value, 'a', e.value_size);
    e.value[e.value_size - 1] = 0;

    return e;
}

static void
benchmark_entry_destroy(struct benchmark_entry *e)
{
    cc_free(e->key);
    cc_free(e->value);
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
}

static struct duration
benchmark_run(struct benchmark *b)
{
    struct array *in;
    struct array *in2;

    struct array *out;

    size_t nentries = O(b, nentries);

    bench_storage_init(O(b, entry_max_size), nentries);

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

            if (bench_storage_get(*e) != CC_OK) {
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

            if (bench_storage_put(*e) != CC_OK) {
                log_info("benchmark put() failed");
            }

            struct benchmark_entry **e2 = array_push(in2);
            *e2 = *e;
        }
        pct_sum += O(b, pct_put);
        if (pct_sum < pct && pct <= O(b, pct_rem) + pct_sum) {
            ASSERT(array_nelem(in) != 0);
            struct benchmark_entry **e = array_pop(in);

            if (bench_storage_rem(*e) != CC_OK) {
                log_info("benchmark rem() failed");
            }

            struct benchmark_entry **e2 = array_push(out);
            *e2 = *e;
        }
    }

    duration_stop(&d);

    bench_storage_deinit();

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
