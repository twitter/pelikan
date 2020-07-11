

#include "bench_storage.h"
#include "reader.h"

#include "cc_mm.h"

#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <unistd.h>
#include <utime.h>


#define BUF_N_ENTRY 8000000
//#define BUF_N_ENTRY  73439682
/* make thread a compile time constant allows loop unrolling */
#define N_THREAD 1



/**
 *  a wrapper for multiple threaded reader,
 *  this is using a multi-producer single consumer model
 **/
struct reader_mt {
    struct reader *reader;
    struct benchmark_entry *e;
    int64_t n_total_req;
    int64_t read_pos;
    int64_t write_pos;
    int64_t *write_pos_thd;
    pthread_t pids[N_THREAD];
};

struct _thread_params {
    struct reader_mt *reader_mt;
    int thread_idx;
};

static void
set_thread_affinity(int core_id)
{
#ifdef __linux__
    cpu_set_t cpuset;
    CPU_ZERO(&cpuset);
    CPU_SET(core_id, &cpuset);

    if (pthread_setaffinity_np(pthread_self(), sizeof(cpu_set_t), &cpuset) != 0)
        printf("pthread_setaffinity_np error");
#endif

    ;
}

static void *
_reader_thread(void *data)
{
    struct _thread_params *params = data;
    struct reader_mt *reader_mt = params->reader_mt;
    int thread_idx = params->thread_idx;
    int64_t *write_pos_thd = &reader_mt->write_pos_thd[thread_idx];
    struct benchmark_entry *e;
    int64_t read_pos;

    set_thread_affinity(thread_idx);

    int eof = 0;
    while (eof == 0) {
        *write_pos_thd =
                __atomic_fetch_add(&reader_mt->write_pos, 1, __ATOMIC_RELAXED);
        read_pos = __atomic_load_n(&reader_mt->read_pos, __ATOMIC_RELAXED);
        while (*write_pos_thd - read_pos >= BUF_N_ENTRY - 2) {
            usleep(1);
            //                        sched_yield();
            read_pos = __atomic_load_n(&reader_mt->read_pos, __ATOMIC_RELAXED);
        }

        e = &reader_mt->e[*write_pos_thd % BUF_N_ENTRY];
        eof = read_trace(reader_mt->reader, &e);
        *write_pos_thd = INT64_MAX;
    }
    printf("reader thread now exit\n");
    return NULL;
}

static inline struct reader_mt *
open_trace_mt(const char *trace_path)
{
    struct reader *reader = open_trace(trace_path);
    struct reader_mt *reader_mt =
            (struct reader_mt *)cc_zalloc(sizeof(struct reader_mt));
    reader_mt->reader = reader;
    size_t sz = sizeof(struct benchmark_entry) * BUF_N_ENTRY;
    reader_mt->write_pos_thd = (int64_t *)cc_zalloc(sizeof(int64_t) * N_THREAD);
    reader_mt->n_total_req = reader->n_total_req;
    struct _thread_params params[N_THREAD];

    set_thread_affinity(N_THREAD + 2);

    reader_mt->e = (struct benchmark_entry *)cc_zalloc(sz);
    ASSERT(reader_mt->e != NULL);
    for (int i = 0; i < BUF_N_ENTRY; i++) {
        reader_mt->e[i].key = cc_zalloc(MAX_KEY_LEN);
        ASSERT(reader_mt->e[i].key != NULL);
    }

    for (int i = 0; i < N_THREAD; i++) {
        params[i].reader_mt = reader_mt;
        params[i].thread_idx = i;
    }

    for (int i = 0; i < N_THREAD; i++) {
        pthread_create(&reader_mt->pids[i], NULL, _reader_thread, &params[i]);
    }

    /* wait for thread to start, this is important since _thread_params
     * are on the stack */
    sleep(1);
    return reader_mt;
}

static inline int
read_trace_mt(struct reader_mt *reader_mt, struct benchmark_entry **e)
{
    int64_t smallest_write_pos = INT64_MAX;
    int64_t read_pos =
            __atomic_fetch_add(&reader_mt->read_pos, 1, __ATOMIC_RELAXED);

    if (read_pos >= reader_mt->n_total_req)
        return 1;

    int64_t tmp;
    for (int i = 0; i < N_THREAD; i++) {
        tmp = __atomic_load_n(&reader_mt->write_pos_thd[i], __ATOMIC_RELAXED);
        if (tmp < smallest_write_pos) {
            smallest_write_pos = tmp;
        }
    }

    while ((read_pos >= smallest_write_pos)) {
        smallest_write_pos = INT64_MAX;
        for (int i = 0; i < N_THREAD; i++) {
            tmp = __atomic_load_n(
                    &reader_mt->write_pos_thd[i], __ATOMIC_RELAXED);
            if (tmp < smallest_write_pos) {
                smallest_write_pos = tmp;
            }
        }
    }

    *e = &reader_mt->e[read_pos % BUF_N_ENTRY];

    return 0;
}

static inline void
close_trace_mt(struct reader_mt *reader_mt)
{
    for (int i = 0; i < N_THREAD; i++)
        pthread_join(reader_mt->pids[i], NULL);


    close_trace(reader_mt->reader);

    cc_free(reader_mt->e);
    cc_free(reader_mt->write_pos_thd);
    cc_free(reader_mt);
}