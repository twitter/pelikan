
/* a reader that preloads all requests into DRAM */


#include "bench_storage.h"
#include "reader.h"

#include "cc_mm.h"

#include <pthread.h>
#include <time.h>
#include <unistd.h>
#include <utime.h>


/**
 *  a wrapper for preloaded reader, it supports multi-threading
 *  note that it does not support trace time
 *
 **/
struct reader_pl {
    struct benchmark_entry *e;
    int64_t n_total_req;
    int64_t read_pos;
};


static inline struct reader_pl *
open_trace_pl(const char *trace_path)
{
    struct reader *reader = open_trace(trace_path);
    struct reader_pl *reader_pl =
            (struct reader_pl *)cc_zalloc(sizeof(struct reader_pl));
    reader_pl->n_total_req = reader->n_total_req;
    size_t sz = sizeof(struct benchmark_entry) * reader->n_total_req;
    reader_pl->e = (struct benchmark_entry *)cc_zalloc(sz);
    ASSERT(reader_pl->e != NULL);

    struct benchmark_entry *e;

    for (int64_t i = 0; i < reader->n_total_req; i++) {
        reader_pl->e[i].key = NULL;
        e = &reader_pl->e[i];
        read_trace(reader, &e);
        ASSERT(reader_pl->e[i].key != NULL);
    }

    printf("preloaded reader load all %" PRId64 " requests from %s\n",
            reader_pl->n_total_req, trace_path);

    close_trace(reader);

    return reader_pl;
}

static inline int
read_trace_pl(struct reader_pl *reader_pl, struct benchmark_entry **e)
{
    int64_t read_pos =
            __atomic_fetch_add(&reader_pl->read_pos, 1, __ATOMIC_RELAXED);

    if (read_pos >= reader_pl->n_total_req)
        return 1;

    *e = &reader_pl->e[read_pos];

    return 0;
}

static inline void
close_trace_pl(struct reader_pl *reader_pl)
{
    for (int64_t i=0; i<reader_pl->n_total_req; i++)
        cc_free(reader_pl->e[i].key);

    cc_free(reader_pl->e);
    cc_free(reader_pl);
}