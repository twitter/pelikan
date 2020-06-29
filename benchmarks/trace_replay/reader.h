#pragma once


#include <stddef.h>
#include <inttypes.h>
#include <stdio.h>


#define MAX_TRACE_PATH_LEN 1024
#define MAX_KEY_LEN 255
#define MAX_VAL_LEN 8 * 1024 * 1024

struct benchmark_entry;


struct reader {
    char *mmap;
    size_t offset;
    size_t file_size;
    char trace_path[MAX_TRACE_PATH_LEN];
    uint64_t n_total_req;

    /* this is not thread-safe, currently a hack to use existing benchmark */
    char curr_key[MAX_KEY_LEN];
};


struct reader *
open_trace(char *trace_path);


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
read_trace(struct reader *reader, struct benchmark_entry *e);


void close_trace(struct reader *reader);