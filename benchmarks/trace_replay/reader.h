#pragma once


#include <inttypes.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdio.h>
#include <time/time.h>


#define MAX_TRACE_PATH_LEN 1024

struct benchmark_entry;


struct reader {
    char *mmap;
    size_t offset;
    size_t file_size;
    char trace_path[MAX_TRACE_PATH_LEN];
    uint64_t n_total_req;
    /* used for preloaded reader */
    struct benchmark_entry *e;
    const int32_t *default_ttls;
    int default_ttl_idx;
    bool update_time; /* whether this reader is responsible for updating time */
    int32_t start_ts;
    int32_t curr_ts;
};


struct reader *
open_trace(const char *trace_path, const int32_t *default_ttls);


/*
 * read one request from trace and store in benchmark_entry
 *
 * current trace format:
 * 20 byte for each request,
 * first 4-byte is time stamp
 * next 8-byte is key encoded using increasing integer sequence
 * next 4-byte is key and val size,
 *      the left 10-bit is key size, right 22-bit is val size
 * next 4-byte is op and ttl,
 *      the left 8-bit is op and right 24-bit is ttl
 *      op is the index in the following array: get, gets, set, add,
 *      cas, replace, append, prepend, delete, incr, decr
 *
 * return 1 on trace EOF, otherwise 0
 *
 */
int
read_trace(struct reader *reader);

void close_trace(struct reader *reader);