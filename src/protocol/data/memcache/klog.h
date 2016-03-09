#pragma once
#include <cc_define.h>
#include <cc_metric.h>

#include <stdint.h>

#define KLOG_NBUF   2 * MiB    /* default log buf size */
#define KLOG_INTVL  100000000  /* flush every 100 milliseconds */
#define KLOG_SAMPLE 100        /* log one in every 100 commands */
#define KLOG_MAX    GiB        /* max klog file size */

/*          name         type              default       description */
#define KLOG_OPTION(ACTION)                                                                                                   \
    ACTION( klog_file,   OPTION_TYPE_STR,  NULL,         "command log file"                                                  )\
    ACTION( klog_backup, OPTION_TYPE_STR,  NULL,         "command log backup file"                                           )\
    ACTION( klog_nbuf,   OPTION_TYPE_UINT, KLOG_NBUF,    "command log buf size"                                              )\
    ACTION( klog_intvl,  OPTION_TYPE_UINT, KLOG_INTVL,   "command log flush interval"                                        )\
    ACTION( klog_sample, OPTION_TYPE_UINT, KLOG_SAMPLE,  "command log sample ratio"                                          )\
    ACTION( klog_max,    OPTION_TYPE_UINT, KLOG_NAX,     "max klog file size - allow up to 2x klog_max disk if using backup" )

/*          name            type            description */
#define KLOG_METRIC(ACTION)                                                  \
    ACTION( klog_logged,    METRIC_COUNTER, "# commands logged"             )\
    ACTION( klog_discard,   METRIC_COUNTER, "# commands discarded"          )\
    ACTION( klog_skip,      METRIC_COUNTER, "# commands skipped (sampling)" )

typedef struct {
    KLOG_METRIC(METRIC_DECLARE)
} klog_metrics_st;

#define KLOG_METRIC_INIT(_metrics) do {                     \
    *(_metrics) =                                           \
        (klog_metrics_st) { KLOG_METRIC(METRIC_INIT) };     \
} while (0)

struct request;
struct response;
struct timeout_event;

extern struct timeout_event *klog_tev;

rstatus_i klog_setup(char *file, char *backup, uint32_t nbuf, uint32_t interval,
                     uint32_t sample, size_t max, klog_metrics_st *metrics);
void klog_teardown(void);

void klog_write(struct request *req, struct response *rsp);
