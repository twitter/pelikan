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

#include <cc_debug.h>

#include <cc_log.h>

#include <cc_print.h>

#include <ctype.h>
#include <errno.h>
#include <execinfo.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define BACKTRACE_DEPTH 64
#define DEBUG_MODULE_NAME "ccommon::debug"

struct logger *debug_logger = NULL;
static bool debug_init = false;

void
debug_assert(const char *cond, const char *file, int line, int panic)
{
    log_stderr("assert '%s' failed @ (%s, %d)", cond, file, line);
    if (panic) {
        debug_stacktrace(1);
        abort();
    }
}

void
debug_stacktrace(int skip_count)
{
#ifdef CC_BACKTRACE
    void *stack[BACKTRACE_DEPTH];
    char **symbols;
    int size, i, j;

    log_stderr("printing stracktrace (depth limit: %d)", BACKTRACE_DEPTH);
    size = backtrace(stack, BACKTRACE_DEPTH);
    symbols = backtrace_symbols(stack, size);
    if (symbols == NULL) {
        log_stderr("Could not print backtrace; no symbols");
        return;
    }

    skip_count++; /* skip the current frame also */

    for (i = skip_count, j = 0; i < size; i++, j++) {
        log_stderr("[%d] %s", j, symbols[i]);
    }

    free(symbols);
#endif
}

rstatus_t
debug_setup(int log_level, char *log_file, uint32_t log_nbuf)
{
    log_stderr("Set up the %s module", DEBUG_MODULE_NAME);

    if (debug_init) {
        log_stderr("%s has already been setup, overwrite", DEBUG_MODULE_NAME);
    }

    if (debug_logger != NULL) {
        log_destroy(&debug_logger);
    }

    debug_logger = log_create(log_level, log_file, log_nbuf);

    if (debug_logger == NULL) {
        log_stderr("Could not create debug logger!");
        return CC_ERROR;
    }

    debug_init = true;

    return CC_OK;
}

void
debug_teardown(void)
{
    log_stderr("Tear down the %s module", DEBUG_MODULE_NAME);

    if (!debug_init) {
        log_stderr("%s was never setup", DEBUG_MODULE_NAME);
    }

    if (debug_logger != NULL) {
        log_destroy(&debug_logger);
    }

    debug_init = false;
}

void
_log(struct logger *logger, const char *file, int line, int level, const char *fmt, ...)
{
    int len, size, errno_save;
    char buf[LOG_MAX_LEN], *timestr;
    va_list args;
    struct tm *local;
    time_t t;

    if (!log_loggable(logger, level)) {
        return;
    }

    errno_save = errno;
    len = 0;            /* length of output buffer */
    size = LOG_MAX_LEN; /* size of output buffer */

    t = time(NULL);
    local = localtime(&t);
    timestr = asctime(local);

    len += cc_scnprintf(buf + len, size - len, "[%.*s] %s:%d ",
                        strlen(timestr) - 1, timestr, file, line);

    va_start(args, fmt);
    len += cc_vscnprintf(buf + len, size - len, fmt, args);
    va_end(args);

    buf[len++] = '\n';

    _log_write(logger, buf, len);

    errno = errno_save;
}
