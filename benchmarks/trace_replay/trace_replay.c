
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

#define READER_TYPE NORMAL_READER


#if !defined(READER_TYPE) || READER_TYPE == NORMAL_READER
#    define OPEN_TRACE(x) open_trace(x)
#    define READ_TRACE(x, e) read_trace(x, e)
#    define CLOSE_TRACE(x) close_trace(x)
#    define CLONE_READER(reader) clone_reader(reader)
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
    ACTION(n_warmup_req, OPTION_TYPE_UINT, 0, "#requests from eval trace used to warmup (per thread)") \
    ACTION(default_ttl, OPTION_TYPE_UINT, 3600, "default ttl for set request with no ttl") \
    ACTION(n_thread, OPTION_TYPE_UINT, 1, "the number of threads")             \
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


volatile static bool start = false;


void
dump_seg_info(void);

static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    memset(b, 0, sizeof(*b));

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

    b->n_thread = O_UINT(b, n_thread);
    b->n_warmup_req = O_UINT(b, n_warmup_req);
    b->default_ttl = O_UINT(b, default_ttl);

    if (O_BOOL(b, debug_logging)) {
        if (debug_setup(&BENCH_OPTS(b)->debug) != CC_OK) {
            log_stderr("debug log setup failed");
            exit(EX_CONFIG);
        }
    }

    b->eval_reader = OPEN_TRACE(O_STR(b, eval_trace_path));
    if (b->eval_reader == NULL) {
        log_stderr("failed to open eval_trace");
        exit(EX_CONFIG);
    }

    b->warmup_reader = NULL;
    if (strlen(O_STR(b, warmup_trace_path)) > 0) {
        b->warmup_reader = OPEN_TRACE(O_STR(b, warmup_trace_path));
        if (b->warmup_reader == NULL) {
            log_stderr("failed to open warmup_trace");
            exit(EX_CONFIG);
        }
    }

    uint64_t nops = ((READER *)b->eval_reader)->n_total_req;

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
    if (O_BOOL(b, per_op_latency)) {
        cc_free(b->latency.samples);
        cc_free(b->latency.ops);
    }

    cc_free(b->options);

    if (b->warmup_reader) {
        CLOSE_TRACE(b->warmup_reader);
    }
    CLOSE_TRACE(b->eval_reader);
}


static struct duration
trace_replay_run(struct benchmark *b)
{
    bool per_op_latency = O_BOOL(b, per_op_latency);

    bench_storage_init(BENCH_OPTS(b)->engine, 0, 0);

    READER *wreader = b->warmup_reader;
    READER *ereader = b->eval_reader;
    struct benchmark_entry *e = NULL;

    if (wreader != NULL) {
        printf("start warmup using warmup trace");
        struct duration wd;
        duration_start(&wd);

        while (READ_TRACE(wreader, &e) == 0) {
            if (e->op != op_incr && e->op != op_decr) {
                e->op = op_set;
                bench_storage_set(e);
            } else if (e->op == op_incr) {
                bench_storage_incr(e);
            } else if (e->op == op_decr) {
                bench_storage_decr(e);
            }
        }
        duration_stop(&wd);

        printf("%.2lf sec warmup finished - %lu requests\n", duration_sec(&wd),
                (unsigned long)wreader->n_total_req);
    }


    struct duration d;
    duration_start(&d);

    rstatus_i status;
    uint64_t n_miss = 0;
    uint64_t n_req = 0;

    while (READ_TRACE(ereader, &e) == 0) {
        if (n_req < b->n_warmup_req){
            e->op = op_set;
            e->expire_at = time_proc_sec() + b->default_ttl;
        }

        status = benchmark_run_operation(b, e, per_op_latency);
        if (status == CC_EEMPTY) {
            n_miss += 1;
        }

        n_req += 1;
        //        if (n_req % 1000000 == 0)
        //            dump_seg_info();
    }

    duration_stop(&d);

    printf("%" PRIu64 " req, %" PRIu64 " miss (%.4f)\n", ereader->n_total_req,
            n_miss, (double)n_miss / ereader->n_total_req);

    bench_storage_deinit();

    return d;
}


static void *
_trace_replay_thread(void *arg)
{
    static __thread uint64_t n_miss = 0;
    static __thread uint64_t n_req = 0;

    struct benchmark *b = arg;

    rstatus_i status;

    READER *ereader = b->eval_reader;
//    ereader = CLONE_READER(ereader);

    struct benchmark_entry *e = cc_zalloc(sizeof(struct benchmark_entry));
    e->key = cc_zalloc(MAX_KEY_LEN);

    while (!start) {
        ;
    }

    while (READ_TRACE(ereader, &e) == 0) {
        if (n_req < b->n_warmup_req){
            e->op = op_set;
            e->expire_at = time_proc_sec() + b->default_ttl;
        }

        status = run_op(e);

        if (status == CC_EEMPTY) {
            n_miss += 1;
        } else if (status == CC_OK) {
            ;
        }
//        else {
//            printf("%p %s other status %d\n", pthread_self(), op_names[e->op], status);
//        }
        n_req += 1;
    }

    cc_free(e->key);
    cc_free(e);

    __atomic_add_fetch(&b->n_req, n_req, __ATOMIC_RELAXED);
    __atomic_add_fetch(&b->n_miss, n_miss, __ATOMIC_RELAXED);

    return NULL;
}

static struct duration
trace_replay_run_mt(struct benchmark *b)
{
    /* disable per op latency when we run in multi-threading mode */
    pthread_t *pids = cc_alloc(sizeof(pthread_t) * b->n_thread);
    READER *wreader = b->warmup_reader;
    struct benchmark_entry *e = NULL;

    bench_storage_init(BENCH_OPTS(b)->engine, 0, 0);

    if (wreader != NULL) {
        printf("start warmup");
        struct duration wd;
        duration_start(&wd);

        while (READ_TRACE(wreader, &e) == 0) {
            if (e->op != op_incr && e->op != op_decr) {
                e->op = op_set;
                bench_storage_set(e);
            } else if (e->op == op_incr) {
                bench_storage_incr(e);
            } else if (e->op == op_decr) {
                bench_storage_decr(e);
            }
        }
        duration_stop(&wd);

        printf("%.2lf sec warmup finished - %lu requests\n", duration_sec(&wd),
                (unsigned long)wreader->n_total_req);
    }

    for (int i = 0; i < b->n_thread; i++) {
        pthread_create(&pids[i], NULL, _trace_replay_thread, b);
    }

    /* wait for eval thread ready */
    sleep(1);
    start = true;

    struct duration d;
    duration_start(&d);

    for (int i = 0; i < b->n_thread; i++) {
        pthread_join(pids[i], NULL);
    }
    duration_stop(&d);

    return d;
}


int
main(int argc, char *argv[])
{
    struct benchmark b;
    struct duration d;
    if (benchmark_create(&b, argv[1]) != 0) {
        loga("failed to create benchmark instance");
        return -1;
    }

    if (b.n_thread == 1) {
        d = trace_replay_run(&b);

        benchmark_print_summary(&b, &d, O_BOOL(&b, per_op_latency));

    } else {
        d = trace_replay_run_mt(&b);

        printf("total benchmark runtime: %f s, throughput %.2f M QPS\n",
                duration_sec(&d), b.n_req / duration_sec(&d) / 1000000);
        printf("average operation latency: %f ns, miss ratio %.4lf\n",
                duration_ns(&d) / b.n_req, (double)b.n_miss / b.n_req);
    }

    benchmark_destroy(&b);

    return 0;
}
