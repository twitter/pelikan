#pragma once

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
    ACTION( process_server_ex, METRIC_COUNTER, "# internal error"      )

typedef struct {
    PROCESS_METRIC(METRIC_DECLARE)
} process_metrics_st;

void process_setup(process_options_st *options, process_metrics_st *metrics);
void process_teardown(void);

int redis_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int redis_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
int redis_process_error(struct buf **rbuf, struct buf **wbuf, void **data);
