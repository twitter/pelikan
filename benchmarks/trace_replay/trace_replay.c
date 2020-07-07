
#include "bench_storage.h"
#include "reader.h"
#include "reader_mt.h"
#include "reader_pl.h"

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_log.h>
#include <cc_mm.h>
#include <time/cc_timer.h>

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

#define NORMAL_READER 1
#define MULTI_THREAD_READER 2
#define PRELOADED_READER 3

#define READER_TYPE PRELOADED_READER

#if !defined(READER_TYPE) || READER_TYPE == NORMAL_READER
#    define OPEN_TRACE(x) open_trace(x)
#    define READ_TRACE(x, e) read_trace(x, e)
#    define CLOSE_TRACE(x) close_trace(x)
#    define READER struct reader

#elif READER_TYPE == MULTI_THREAD_READER
#    define OPEN_TRACE(x) open_trace_mt(x)
#    define READ_TRACE(x, e) read_trace_mt(x, e)
#    define CLOSE_TRACE(x) close_trace_mt(x)
#    define READER struct reader_mt

#elif READER_TYPE == PRELOADED_READER
#    define OPEN_TRACE(x) open_trace_pl(x)
#    define READ_TRACE(x, e) read_trace_pl(x, e)
#    define CLOSE_TRACE(x) close_trace_pl(x)
#    define READER struct reader_pl

#endif


#define BENCHMARK_OPTION(ACTION)                                               \
    ACTION(warmup_trace_path, OPTION_TYPE_STR, "", "path to the trace")        \
    ACTION(eval_trace_path, OPTION_TYPE_STR, "trace.bin", "path to the trace") \
    ACTION(per_op_latency, OPTION_TYPE_BOOL, true, "Collect latency samples")  \
    ACTION(debug_logging, OPTION_TYPE_BOOL, true, "turn on debug logging")


struct replay_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct replay_specific benchmark;
    debug_options_st debug;
    struct option engine[]; /* storage-engine specific options... */
};


static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    memset(b, 0, sizeof(*b));
    //    b->entries = cc_zalloc(sizeof(struct benchmark_entry) * 1);
    //    b->entries->key = cc_zalloc(MAX_KEY_LEN);
    //    ASSERT(b->entries->key != NULL);

    unsigned nopts = OPTION_CARDINALITY(struct replay_specific);

    struct replay_specific replay_opts = {BENCHMARK_OPTION(OPTION_INIT)};
    option_load_default((struct option *)&replay_opts, nopts);

    debug_options_st debug_opts = {DEBUG_OPTION(OPTION_INIT)};
    nopts += OPTION_CARDINALITY(debug_options_st);
    option_load_default(
            (struct option *)&debug_opts, OPTION_CARDINALITY(debug_options_st));

    nopts += bench_storage_config_nopts();

    b->options = cc_alloc(sizeof(struct option) * nopts);
    ASSERT(b->options != NULL);
    BENCH_OPTS(b)->benchmark = replay_opts;
    BENCH_OPTS(b)->debug = debug_opts;

    bench_storage_config_init(BENCH_OPTS(b)->engine);


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
        if (debug_setup(&BENCH_OPTS(b)->debug) != CC_OK) {
            log_stderr("debug log setup failed");
            exit(EX_CONFIG);
        }
    }

    b->reader = OPEN_TRACE(O_STR(b, eval_trace_path));
    if (b->reader == NULL) {
        log_stderr("failed to open trace");
        exit(EX_CONFIG);
    }

    uint64_t nops = ((READER *)b->reader)->n_total_req;

    if (O_BOOL(b, per_op_latency)) {
        b->latency.samples = cc_zalloc(nops * sizeof(struct duration));
        ASSERT(b->latency.samples != NULL);

        b->latency.ops = cc_zalloc(nops * sizeof(op_e));
        ASSERT(b->latency.ops != NULL);
    }

    b->latency.count = 0;

    return CC_OK;
}

static void
benchmark_destroy(struct benchmark *b)
{
    //    cc_free(b->entries->key);
    //    cc_free(b->entries);

    if (O_BOOL(b, per_op_latency)) {
        cc_free(b->latency.samples);
        cc_free(b->latency.ops);
    }

    cc_free(b->options);

    CLOSE_TRACE(b->reader);
}


static struct duration
trace_replay_run(struct benchmark *b)
{
    bool per_op_latency = O_BOOL(b, per_op_latency);

    bench_storage_init(BENCH_OPTS(b)->engine, 0, 0);

    READER *reader = b->reader;
    struct benchmark_entry *e = NULL;

    rstatus_i status;
    uint64_t n_miss = 0;
    uint64_t n_req = reader->n_total_req;

    struct duration d;
    duration_start(&d);

    while (READ_TRACE(reader, &e) == 0) {
        status = benchmark_run_operation(b, e, per_op_latency);
        /* we are counting read-after-delete as miss, maybe exclude this */
        if (status == CC_EEMPTY) {
            n_miss += 1;
        }
    }

    duration_stop(&d);

    printf("%" PRIu64 " req, %" PRIu64 " miss (%.4f)\n", n_req, n_miss,
            (double)n_miss / n_req);

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

    struct duration d = trace_replay_run(&b);

    benchmark_print_summary(&b, &d, O_BOOL(&b, per_op_latency));

    benchmark_destroy(&b);

    return 0;
}
