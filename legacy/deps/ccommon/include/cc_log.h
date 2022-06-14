/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_define.h>
#include <cc_metric.h>
#include <cc_util.h>

#include <stdbool.h>

#define LOG_MAX_LEN 2560 /* max length of log message to STDOUT/STDERR */

struct logger {
    char *name;                 /* log file name */
    int  fd;                    /* log file descriptor */
    struct rbuf *buf;           /* ring buffer for pauseless logging */
};

/*          name            type            description */
#define LOG_METRIC(ACTION)                                                      \
    ACTION( log_create,     METRIC_COUNTER, "# loggers created"                )\
    ACTION( log_create_ex,  METRIC_COUNTER, "# log create errors"              )\
    ACTION( log_destroy,    METRIC_COUNTER, "# loggers destroyed"              )\
    ACTION( log_curr,       METRIC_GAUGE,   "current # loggers"                )\
    ACTION( log_open,       METRIC_COUNTER, "# files opened by loggers"        )\
    ACTION( log_open_ex,    METRIC_COUNTER, "# logger open file errors"        )\
    ACTION( log_write,      METRIC_COUNTER, "# log messages written"           )\
    ACTION( log_write_byte, METRIC_COUNTER, "# bytes written by log"           )\
    ACTION( log_write_ex,   METRIC_COUNTER, "# log write errors"               )\
    ACTION( log_skip,       METRIC_COUNTER, "# messages not completely logged" )\
    ACTION( log_skip_byte,  METRIC_COUNTER, "# bytes unable to be logged"      )\
    ACTION( log_flush,      METRIC_COUNTER, "# log flushes to disk"            )\
    ACTION( log_flush_ex,   METRIC_COUNTER, "# errors flushing to disk"        )

typedef struct {
    LOG_METRIC(METRIC_DECLARE)
} log_metrics_st;

#define log_stderr(...) _log_fd(STDERR_FILENO, __VA_ARGS__)
#define log_stdout(...) _log_fd(STDOUT_FILENO, __VA_ARGS__)

void log_setup(log_metrics_st *metrics);
void log_teardown(void);

/* these two are for testing purposes only */
log_metrics_st *log_metrics_create(void);
void log_metrics_destroy(log_metrics_st **p);

/**
 * Create a logger. If filename is NULL, created logger writes to stderr.
 * buf_cap is the size of the buffer used for pauseless logging. specify
 * buf_cap = 0 to disable pauseless logging.
 */
struct logger *log_create(char *filename, uint32_t buf_cap);

void log_destroy(struct logger **logger);

/**
 * Reopen the log file. Optional argument target - if left NULL, log_reopen
 * will simply reopen the log file. If specified, log_reopen will rename the
 * original log file to the provided target filename, and reopen the log file.
 */
rstatus_i log_reopen(struct logger *logger, char *target);

/* _log_write returns true if msg written, false if skipped or failed */
bool log_write(struct logger *logger, char *buf, uint32_t len);

void _log_fd(int fd, const char *fmt, ...);

size_t log_flush(struct logger *logger);

#ifdef __cplusplus
}
#endif
