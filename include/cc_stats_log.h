#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_metric.h>
#include <cc_option.h>


#define STATS_LOG_FILE  NULL    /* default log file */
#define STATS_LOG_NBUF  0       /* default log buf size */

/*          name            type              default           description */
#define STATS_LOG_OPTION(ACTION)                                                         \
    ACTION( stats_log_file, OPTION_TYPE_STR,  NULL,             "file storing stats"   )\
    ACTION( stats_log_nbuf, OPTION_TYPE_UINT, STATS_LOG_NBUF,   "stats log buf size"   )

typedef struct {
    STATS_LOG_OPTION(OPTION_DECLARE)
} stats_log_options_st;


/* dump stats as CSV records into a log file, this allows metrics to be captured
 * locally without setting up an observability infrastructure
 */
void stats_log_setup(stats_log_options_st *options);
void stats_log_teardown(void);

void stats_log(struct metric metrics[], unsigned int nmetric);

void stats_log_flush(void);


#ifdef __cplusplus
}
#endif

