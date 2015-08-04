#pragma once

#include <cc_define.h>

#include <stdint.h>

#define KLOG_NBUF  16 * KiB /* default log buf size */
#define KLOG_INTVL 100000   /* flush every 100 milliseconds */

struct request;

/*          name             type              default               description */
#define KLOG_OPTION(ACTION) \
    ACTION( klog_file,  OPTION_TYPE_STR,  NULL,            "command log file"           )\
    ACTION( klog_nbuf,  OPTION_TYPE_UINT, str(KLOG_NBUF),  "command log buf size"       )\
    ACTION( klog_intvl, OPTION_TYPE_UINT, str(KLOG_INTVL), "command log flush interval" )

rstatus_t klog_setup(char *file, uint32_t nbuf, uint32_t interval);
void klog_teardown(void);

void klog_write(struct request *req, int status, uint32_t rsp_len);
