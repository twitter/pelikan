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


/* pseudo random number generator */
volatile static bool        start = false;
static __thread __uint128_t g_lehmer64_state;
static char                 val_array[MAX_VAL_LEN];
static size_t               n_entries;
static size_t               n_ops;
static size_t               entry_size;
static op_e                 op;


#define RRAND(min, max) (rand_r(&(rseed)) % ((max) - (min) + 1) + (min))


#define BENCHMARK_OPTION(ACTION)                                                                \
    ACTION(entry_size,      OPTION_TYPE_UINT, 64,       "The size of cache entry")              \
    ACTION(nentries,        OPTION_TYPE_UINT, 65536,    "Max number of cache entries")          \
    ACTION(nops,            OPTION_TYPE_UINT, 65536,    "Total number of operations")           \
    ACTION(op,              OPTION_TYPE_UINT, 0,        "operation to benchmark, see enum op_e")\
    ACTION(n_thread,        OPTION_TYPE_UINT, 1,        "the number of threads")                \
    ACTION(debug_logging,   OPTION_TYPE_BOOL, false,    "turn on debug logging")

struct benchmark_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct benchmark_specific benchmark;
    debug_options_st debug;
    struct option engine[]; /* storage-engine specific options... */
};
typedef struct benchmark_options bench_options_st;


static inline uint64_t prand(void) {
    g_lehmer64_state *= 0xda942042e4dd58b5;
    return g_lehmer64_state >> 64u;
}


static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    cc_memset(val_array, 'A', MAX_VAL_LEN);
    for (int i=0; i<MAX_VAL_LEN; i++)
        val_array[i] = (char) ('A' + i % 26);

    unsigned n_opts_all, n_opts_bench, n_opts_dbg, n_opts_storage;
    struct benchmark_specific bench_opts = {BENCHMARK_OPTION(OPTION_INIT)};
    debug_options_st debug_opts = {DEBUG_OPTION(OPTION_INIT)};

    n_opts_bench    = OPTION_CARDINALITY(struct benchmark_specific);
    n_opts_dbg      = OPTION_CARDINALITY(debug_options_st);
    n_opts_storage  = bench_storage_config_nopts();
    n_opts_all = n_opts_bench + n_opts_dbg + n_opts_storage;

    b->options = cc_alloc(sizeof(struct option) * n_opts_all);
    ASSERT(b->options != NULL);

    option_load_default((struct option *)&bench_opts, n_opts_bench);
    option_load_default((struct option *)&debug_opts, n_opts_dbg);

    BENCH_OPTS(b)->benchmark = bench_opts;
    BENCH_OPTS(b)->debug = debug_opts;
    bench_storage_config_init(BENCH_OPTS(b)->engine);


    if (config != NULL) {
        FILE *fp = fopen(config, "r");
        if (fp == NULL) {
            exit(EX_CONFIG);
        }
        option_load_file(fp, (struct option *)b->options, n_opts_all);
        fclose(fp);
    }

    if (O_BOOL(b, debug_logging)) {
        if (debug_setup(&(BENCH_OPTS(b)->debug)) != CC_OK) {
            log_stderr("debug log setup failed");
            exit(EX_CONFIG);
        }
    }

    n_entries = O(b, nentries);
    n_ops = O(b, nops);
    op = O(b, op);
    entry_size = O(b, entry_size);

    if (entry_size <= KEY_LEN) {
        log_crit("entry_min_size must larger than %lu", KEY_LEN);
        cc_free(b->options);

        return CC_EINVAL;
    }

    b->entries = cc_alloc(sizeof(struct benchmark_entry) * 1);
    ASSERT(b->entries != NULL);
    b->entries[0].key_len = KEY_LEN;      /* KEY_LEN needs to be large enough */

    b->entries[0].val = val_array;
    b->entries[0].val_len = entry_size - KEY_LEN;
    b->entries[0].expire_at = INT32_MAX;

    return CC_OK;
}

static void
benchmark_destroy(struct benchmark *b)
{
    cc_free(b->options);
    cc_free(b->entries);
}

static void warm_up(struct benchmark *b, bool val_num) {
    struct benchmark_entry *e = b->entries;
    rstatus_i status;
    char val[32];
    int len;

    for (uint32_t key = 1; key <= (uint32_t) n_entries; key++) {
        snprintf(e->key, e->key_len, "%.*lu", KEY_LEN-1, (unsigned long) key);
        if (val_num) {
            len = sprintf(val, "%lu", (unsigned long) key);
            e->val = val;
            e->val_len = len;
        }
        status = bench_storage_set(e);
        ASSERT(status == CC_OK);
    }
}

static void log_msg(char *msg) {
    time_t timer = time(NULL);
    struct tm* tm_info = localtime(&timer);
    static char buffer[128];
    strftime(buffer, 26, "%Y-%m-%d %H:%M:%S", tm_info);
    printf("%s %s\n", buffer, msg);
}

static struct duration
benchmark_run(struct benchmark *b)
{
//    log_msg("bechmark start");

    rstatus_i (*bench_func)(struct benchmark_entry*);
    struct benchmark_entry *e = b->entries;

    switch (op) {
    case op_get:
        /* make sure we do not get cache miss */
        bench_storage_init(BENCH_OPTS(b)->engine, entry_size, n_entries * 2);
        bench_func = bench_storage_get;
        warm_up(b, false);
        break;
    case op_set:
        bench_storage_init(BENCH_OPTS(b)->engine, entry_size, n_entries);
        bench_func = bench_storage_set;
        warm_up(b, false);
        break;
    case op_cas:
        bench_storage_init(BENCH_OPTS(b)->engine, entry_size, n_entries);
        bench_func = bench_storage_cas;
        warm_up(b, false);
        break;
    case op_incr:
        bench_storage_init(BENCH_OPTS(b)->engine, entry_size, n_entries);
        bench_func = bench_storage_incr;
        warm_up(b, true);
        e->delta = 1;
        break;
    case op_delete:
        bench_storage_init(BENCH_OPTS(b)->engine, entry_size, n_entries);
        bench_func = bench_storage_delete;
//        if (n_ops > n_entries) {
//            printf("delete nops larger than nentries %zu > %zu\n", n_ops, n_entries);
//            exit(EX_CONFIG);
//        }
        warm_up(b, false);
        break;
    default:
        printf("operation %s is not supported\n", op_names[op]);
        exit(EX_CONFIG);
    }

//    log_msg("warmup finish");

    struct duration d;
    duration_start(&d);
    rstatus_i status;
    uint64_t n_fail = 0;

    for (size_t i = 0; i < n_ops; ++i) {
        snprintf(e->key, e->key_len, "%.*"PRIu64, KEY_LEN-1, prand() % n_entries+1);
//        snprintf(e->key, e->key_len, "%.*"PRIu64, KEY_LEN-1, i+1);
        status = bench_func(e);
        if (status != CC_OK) {
            n_fail += 1;
        }
    }

    if (n_fail  > 200)
        printf("%d %.4lf failed\n", n_fail, (double) n_fail/n_ops);


    duration_stop(&d);

    bench_storage_deinit();

    return d;
}


int
main(int argc, char *argv[])
{
    srand(time(NULL));
    g_lehmer64_state = rand();

    struct benchmark b;
    if (benchmark_create(&b, argv[1]) != 0) {
        printf("failed to create benchmark instance\n");
        return -1;
    }

    struct duration d = benchmark_run(&b);

    printf("%20s bench %8s, %8zu entries, %8zu ops, entry_size %8zu, "
           "total benchmark runtime: %8.4f s, "
           "throughput %.2f M QPS, average operation latency: %.0f ns\n",
            argv[0], op_names[op], n_entries, n_ops, entry_size, duration_sec(&d),
            n_ops / duration_sec(&d) / 1000000, duration_ns(&d) / n_ops);


    benchmark_destroy(&b);

    return 0;
}
