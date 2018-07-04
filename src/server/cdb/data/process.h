#pragma once

#include "storage/cdb/cdb.h"

#include <buffer/cc_buf.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <stream/cc_sockio.h>

#define ALLOW_FLUSH false

/*          name         type              default      description */
#define PROCESS_OPTION(ACTION)                                                              \
    ACTION( allow_flush, OPTION_TYPE_BOOL, ALLOW_FLUSH, "allow flushing on the data port"  )

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
    ACTION( gets,              METRIC_COUNTER, "# gets requests"       )\
    ACTION( gets_key,          METRIC_COUNTER, "# keys by gets"        )\
    ACTION( gets_key_hit,      METRIC_COUNTER, "# key hits by gets"    )\
    ACTION( gets_key_miss,     METRIC_COUNTER, "# key misses by gets"  )\
    ACTION( gets_ex,           METRIC_COUNTER, "# gets errors"         )\
    ACTION( invalid,           METRIC_COUNTER, "# invalid command"     )\
    ACTION( flush,             METRIC_COUNTER, "# flush_all requests"  )

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

void process_setup(process_options_st *options, process_metrics_st *metrics, struct CDBHandle *cdb_handle);
void process_teardown(void);

int cdb_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int cdb_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
int cdb_process_error(struct buf **rbuf, struct buf **wbuf, void **data);
