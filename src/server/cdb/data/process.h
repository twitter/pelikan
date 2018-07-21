#pragma once

#include "storage/cdb/cdb_rs.h"

#include <buffer/cc_buf.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <stream/cc_sockio.h>

#define RSP_VAL_BUF_SIZE 1048576

/*          name         type              default      description */
#define PROCESS_OPTION(ACTION)                                                                          \
ACTION( vbuf_size,   OPTION_TYPE_UINT, RSP_VAL_BUF_SIZE, "size in bytes of the value response buffer" )

typedef struct {
    PROCESS_OPTION(OPTION_DECLARE)
} process_options_st;

/*          name                        type            description */
#define PROCESS_METRIC(ACTION)                                          \
    ACTION( process_req,       METRIC_COUNTER, "# requests processed"  )\
    ACTION( process_ex,        METRIC_COUNTER, "# processing error"    )\
    ACTION( process_server_ex, METRIC_COUNTER, "# internal error"      )\
    ACTION( get,               METRIC_COUNTER, "# get requests"        )\
    ACTION( get_key,           METRIC_COUNTER, "# keys by get"         )\
    ACTION( get_key_hit,       METRIC_COUNTER, "# key hits by get"     )\
    ACTION( get_key_miss,      METRIC_COUNTER, "# key misses by get"   )\
    ACTION( get_ex,            METRIC_COUNTER, "# get errors"          )\
    ACTION( invalid,           METRIC_COUNTER, "# invalid command"     )

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

void process_setup(process_options_st *options, process_metrics_st *metrics, struct cdb_handle *cdb_handle);
void process_teardown(void);

int cdb_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int cdb_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
int cdb_process_error(struct buf **rbuf, struct buf **wbuf, void **data);
