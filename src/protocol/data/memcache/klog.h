#pragma once

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_option.h>

#include <stdint.h>

#define KLOG_NBUF   2 * MiB    /* default log buf size */
#define KLOG_INTVL  100        /* flush every 100 milliseconds */
#define KLOG_SAMPLE 100        /* log one in every 100 commands */
#define KLOG_MAX    GiB        /* max klog file size */

/*          name         type              default       description */
#define KLOG_OPTION(ACTION)                                                                                     \
    ACTION( klog_file,   OPTION_TYPE_STR,  NULL,         "command log file"                                    )\
    ACTION( klog_backup, OPTION_TYPE_STR,  NULL,         "command log backup file"                             )\
    ACTION( klog_nbuf,   OPTION_TYPE_UINT, KLOG_NBUF,    "command log buf size"                                )\
    ACTION( klog_intvl,  OPTION_TYPE_UINT, KLOG_INTVL,   "command log flush interval in ms"                    )\
    ACTION( klog_sample, OPTION_TYPE_UINT, KLOG_SAMPLE,  "command log sample ratio"                            )\
    ACTION( klog_max,    OPTION_TYPE_UINT, KLOG_MAX,     "max klog file size - disk usage doubles with backup" )

typedef struct {
    KLOG_OPTION(OPTION_DECLARE)
} klog_options_st;

/*          name            type            description */
#define KLOG_METRIC(ACTION)                                                  \
    ACTION( klog_logged,    METRIC_COUNTER, "# commands logged"             )\
    ACTION( klog_discard,   METRIC_COUNTER, "# commands discarded"          )\
    ACTION( klog_skip,      METRIC_COUNTER, "# commands skipped (sampling)" )

typedef struct {
    KLOG_METRIC(METRIC_DECLARE)
} klog_metrics_st;

struct request;
struct response;
struct timeout_event;

extern bool klog_enabled;
extern struct timeout_event *klog_tev;

void klog_setup(klog_options_st *options, klog_metrics_st *metrics);
void klog_teardown(void);

#define klog_write(req, rsp) do {       \
    if (klog_enabled) {                 \
        _klog_write(req, rsp);          \
    }                                   \
} while (0)

void _klog_write(struct request *req, struct response *rsp);

