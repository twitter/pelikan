
#include "bench_storage.h"
#include "reader.h"
#include "storage/seg/checker.h"

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
#include <sched.h>
#include <pthread.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>

#define N_MAX_THREAD        128

volatile static bool        start = false;
volatile static bool        stop = false;
static char                 val_array[MAX_VAL_LEN];
static int                  n_thread;
static struct reader        *readers[N_MAX_THREAD];

volatile int64_t            op_cnt[op_invalid];
volatile static uint64_t    n_req = 0;
volatile static uint64_t    n_get_req = 0;
volatile static uint64_t    n_miss = 0;

static delta_time_i         default_ttls[100];

extern seg_metrics_st *seg_metrics;


#define BENCHMARK_OPTION(ACTION)                                                                    \
    ACTION(trace_path,      OPTION_TYPE_STR,    NULL,           "path to the trace")                        \
    ACTION(default_ttl_list,OPTION_TYPE_STR,    "86400:1",      "a comma separated list of ttl:percent")    \
    ACTION(n_thread,        OPTION_TYPE_UINT,   1,              "the number of threads")                    \
    ACTION(debug_logging,   OPTION_TYPE_BOOL,   true,           "turn on debug logging")


struct replay_specific {
    BENCHMARK_OPTION(OPTION_DECLARE)
};

struct benchmark_options {
    struct replay_specific benchmark;
    debug_options_st debug;
    struct option engine[]; /* storage-engine specific options... */
};
typedef struct benchmark_options bench_options_st;


static rstatus_i
benchmark_create(struct benchmark *b, const char *config)
{
    cc_memset(val_array, 'A', MAX_VAL_LEN);
    for (int i = 0; i < MAX_VAL_LEN; i++)
        val_array[i] = (char)('A' + i % 26);

    unsigned n_opts_all, n_opts_bench, n_opts_dbg, n_opts_storage;
    struct replay_specific bench_opts = {BENCHMARK_OPTION(OPTION_INIT)};
    debug_options_st debug_opts = {DEBUG_OPTION(OPTION_INIT)};

    n_opts_bench = OPTION_CARDINALITY(struct replay_specific);
    n_opts_dbg = OPTION_CARDINALITY(debug_options_st);
    n_opts_storage = bench_storage_config_nopts();
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

    char *list_start = O_STR(b, default_ttl_list);
    char *curr = list_start;
    char *new_pos;
    delta_time_i ttl;
    double perc;
    int ttl_array_idx = 0;
    while (curr != NULL) {
        ttl = strtol(curr, &new_pos, 10);
        curr = new_pos;
        new_pos = strchr(curr, ':');
        ASSERT(new_pos != NULL);
        curr = new_pos + 1;
        perc = strtod(curr, &new_pos);
        for (int i = 0; i < (int)(perc*100); i++) {
            default_ttls[ttl_array_idx + i] = ttl;
        }
        ttl_array_idx += (int)(perc*100);
        printf("find TTL %"PRId32 ": perc %.4lf, ", ttl, perc);
        curr = new_pos;
        new_pos = strchr(curr, ',');
        curr = new_pos == NULL? NULL: new_pos + 1;
    }
    printf("\n");

    if (ttl_array_idx != 100) {
        ASSERT(ttl_array_idx == 99);
        default_ttls[99] = default_ttls[98];
    }

    n_thread = O_UINT(b, n_thread);

    if (n_thread > 1) {
        char path[MAX_TRACE_PATH_LEN];
        for (int i = 0; i < n_thread; i++) {
            sprintf(path, "%s.%d", O_STR(b, trace_path), i);
            readers[i] = open_trace(path, default_ttls);
            if (readers[i] == NULL) {
                printf("failed to open trace %s\n", path);
                exit(EX_CONFIG);
            }
        }
    } else {
        readers[0] = open_trace(O_STR(b, trace_path), default_ttls);
        if (readers[0] == NULL) {
            printf("failed to open trace %s\n", O_STR(b, trace_path));
            exit(EX_CONFIG);
        }
    }

    return CC_OK;
}

static void
benchmark_destroy(struct benchmark *b)
{
    cc_free(b->options);
    for (int i = 0; i < n_thread; i++) {
        close_trace(readers[i]);
    }
}


static struct duration
trace_replay_run()
{
    struct reader *reader = readers[0];
    reader->update_time = true;
    struct benchmark_entry *e = reader->e;

    struct duration d;
    duration_start(&d);

    rstatus_i status;

    uint64_t dump_start = 20000000, dump_intvl = 2000000;
    //    uint64_t dump_start = 1, dump_intvl = 2000000;
    if (reader->n_total_req > 20000000 * 100) {
        dump_start = 200000000;
        dump_intvl = 20000000;
    }

    while (read_trace(reader) == 0) {
        if (e->op == op_incr || e->op == op_decr) {
            e->op = op_get;
        }
        status = run_op(e);
        op_cnt[e->op] += 1;

        if (e->op == op_get) {
            n_get_req += 1;

            if (status == CC_EEMPTY) {
                n_miss += 1;
                if (e->val_len != 0) {
                    op_cnt[op_set] += 1;
                    e->op = op_set;
                    run_op(e);
                    n_req += 1;
                }
            }
        }

        n_req += 1;
//        if (n_req % 100000 == 0)
//            printf("%ld req\n", n_req);
        //        if (n_req >= dump_start && n_req % dump_intvl == 0){
        //            dump_seg_info();
        //            printf("\n");
        //        }
    }

    duration_stop(&d);

//        printf("metrics evict %ld merge %ld\n",
//                seg_metrics->seg_evict.gauge,
//                seg_metrics->seg_merge.gauge);

//    for (int i = 0; i < 1000; i++)
//        seg_print_warn(i);

    return d;
}

static void *
_time_update_thread(void *arg)
{
    proc_sec = 0;
    bool stop_local = __atomic_load_n(&stop, __ATOMIC_RELAXED);
    while (!stop_local) {
        int32_t min_ts = readers[0]->curr_ts;
        for (int i = 0; i < n_thread; i++) {
            if (readers[i]->curr_ts < min_ts) {
                min_ts = readers[i]->curr_ts;
            }
        }
        if (proc_sec < min_ts) {
            __atomic_store_n(&proc_sec, min_ts, __ATOMIC_RELAXED);
            if (min_ts % 200 == 0) {
                printf("curr sec %d\n", min_ts);
            }
        }
        proc_sec = min_ts;
        usleep(20);
        stop_local = __atomic_load_n(&stop, __ATOMIC_RELAXED);
    }

    printf("end time %d\n", proc_sec);
    return NULL;
}


static void *
_trace_replay_thread(void *arg)
{
    static __thread uint64_t local_n_miss = 0;
    static __thread uint64_t local_n_get_req = 0;
    static __thread uint64_t local_n_req = 0;
    static __thread uint64_t local_op_cnt[op_invalid] = {0};

    int idx = (int) arg;

#ifndef __APPLE__
      /* bind worker to the core */
      cpu_set_t cpuset;
      pthread_t thread = pthread_self();

      CPU_ZERO(&cpuset);
      CPU_SET(idx, &cpuset);

      if (pthread_setaffinity_np(thread, sizeof(cpu_set_t), &cpuset) != 0) {
          printf("fail to bind worker thread to core %d: %s\n",
                 idx, strerror(errno));
      } else {
        printf("binding worker thread to core %d\n", idx);
      }

#endif

    struct reader *reader = readers[idx];
    struct benchmark_entry *e = reader->e;

    rstatus_i status;


    while (!start) {
        ;
    }

    while (read_trace(reader) == 0) {
        status = run_op(e);
        local_op_cnt[e->op] += 1;

        if (e->op == op_get) {
            local_n_get_req += 1;

            if (status == CC_EEMPTY) {
                local_n_miss += 1;

                if (e->val_len != 0) {
                    local_op_cnt[op_set] += 1;
                    e->op = op_set;
                    run_op(e);
                    local_n_req += 1;
                }
            }
        }

        local_n_req += 1;
    }

    __atomic_add_fetch(&n_req, local_n_req, __ATOMIC_RELAXED);
    __atomic_add_fetch(&n_get_req, local_n_get_req, __ATOMIC_RELAXED);
    __atomic_add_fetch(&n_miss, local_n_miss, __ATOMIC_RELAXED);
    for (int i = 0; i < op_invalid; i++) {
        __atomic_add_fetch(&op_cnt[i], local_op_cnt[i], __ATOMIC_RELAXED);
    }

    return NULL;
}

static struct duration
trace_replay_run_mt(struct benchmark *b)
{
    pthread_t time_update_tid;
    pthread_t pids[N_MAX_THREAD];

    pthread_create(&time_update_tid, NULL, _time_update_thread, NULL);

    for (int i = 0; i < n_thread; i++) {
        readers[i]->update_time = false;
        pthread_create(&pids[i], NULL, _trace_replay_thread, (void*) (unsigned long) i);
    }

    /* wait for eval thread ready */
    sleep(1);
    start = true;

    struct duration d;
    duration_start(&d);

    for (int i = 0; i < n_thread; i++) {
        pthread_join(pids[i], NULL);
    }
    duration_stop(&d);

    stop = true;
    pthread_join(time_update_tid, NULL);

    return d;
}


int
main(int argc, char *argv[])
{
    printf("item header %d bytes\n", ITEM_HDR_SIZE);
    struct benchmark b;
    struct duration d;
    if (benchmark_create(&b, argv[1]) != 0) {
        loga("failed to create benchmark instance");
        return -1;
    }

    bench_storage_init(BENCH_OPTS(&b)->engine, 0, 0);

    if (n_thread == 1) {
        d = trace_replay_run();
    } else {
        d = trace_replay_run_mt(&b);
    }


    printf("%s total benchmark runtime: %.2lf s, throughput %.2lf M QPS\n",
            argv[1], duration_sec(&d), n_req / duration_sec(&d) / 1000000);
    printf("average operation latency: %.2lf ns, miss ratio %.4lf\n",
            duration_ns(&d) / n_req, (double)n_miss / n_get_req);

    for (op_e op = op_get; op < op_invalid; op++) {
        if (op_cnt[op] == 0)
            continue;
        printf("op %16s %16"PRIu64 "(%.4lf)\n", op_names[op], op_cnt[op],
                (double)op_cnt[op]/n_req);
    }

    benchmark_destroy(&b);
    bench_storage_deinit();

    return 0;
}
