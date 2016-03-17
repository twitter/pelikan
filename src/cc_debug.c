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
#include <cc_mm.h>
#include <cc_print.h>
#include <time/cc_wheel.h>

#include <ctype.h>
#include <errno.h>
#ifdef CC_BACKTRACE
#include <execinfo.h>
#endif /* CC_BACKTRACE */
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define BACKTRACE_DEPTH 64
#define DEBUG_MODULE_NAME "ccommon::debug"

struct debug_logger default_logger;
struct debug_logger *dlog = &default_logger;
struct timeout_event *dlog_tev;
static bool debug_init = false;
static char * level_str[] = {
    "ALWAYS",
    "CRIT",
    "ERROR",
    "WARN",
    "INFO",
    "DEBUG",
    "VERB",
    "VVERB"
};

static void
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

void
debug_assert(const char *cond, const char *file, int line, int panic)
{
    log_stderr("assert '%s' failed @ (%s, %d)", cond, file, line);
    if (panic) {
        debug_stacktrace(1);
        abort();
    }
}

static void
_stacktrace(int signo)
{
    debug_stacktrace(2); /* skipping functions inside signal module */
    raise(signo);
}

/* this only works on the default handler given the sig_fn format */
static void
_logrotate(int signo)
{
    log_info("received signal %d, reopen log file", signo);
    log_reopen(dlog->logger, NULL);
}

static void
_debug_log_flush(void *arg)
{
    log_flush(dlog->logger);
}

rstatus_i
debug_setup(debug_options_st *options)
{
    size_t log_nbuf = DEBUG_LOG_NBUF;
    uint64_t log_intvl = DEBUG_LOG_INTVL;
    char *filename = DEBUG_LOG_FILE;

    /* since logs are not setup yet, we have to log to stderr */
    log_stderr("Set up the %s module", DEBUG_MODULE_NAME);

    if (debug_init) {
        log_stderr("%s has already been setup, overwrite", DEBUG_MODULE_NAME);
        if (dlog->logger != NULL) {
            log_destroy(&dlog->logger);
        }
    }

    dlog->level = DEBUG_LOG_LEVEL;
    if (options != NULL) {
        filename = option_str(&options->debug_log_file);
        log_nbuf = option_uint(&options->debug_log_nbuf);
        log_intvl = option_uint(&options->debug_log_intvl);
        dlog->level = option_uint(&options->debug_log_level);
    }

    dlog->logger = log_create(filename, log_nbuf);
    if (dlog->logger == NULL) {
        log_stderr("Could not create logger");
        goto error;
    }

    /*
     * 0 length buffer indicates that the logger will log directly to the file,
     * thus we do not need to setup periodic flushing
     */
    if (log_nbuf == 0) {
        goto done;
    }

    if (log_intvl == 0) {
        log_stderr("invalid debug log configuration - debug_log_intvl must"
                   "be non-zero for pauseless logging");
    }

    dlog_tev = timeout_event_create();
    if (dlog_tev == NULL) {
        log_stderr("Could not create timeout event for debug logger");
        goto error;
    }
    dlog_tev->cb = &_debug_log_flush;
    dlog_tev->recur = true;
    timeout_set_ms(&dlog_tev->delay, log_intvl);

done:
    /* some adjustment on signal handling */
    if (signal_override(SIGSEGV, "printing stacktrace when segfault", 0, 0,
            _stacktrace) < 0) {
        goto error;
    }

    /* override the SIGHUP signal to allow nocopytruncate style rotation of logs */
    if (signal_override(SIGHUP, "reopen log file", 0, 0, _logrotate) < 0) {
        goto error;
    }

    debug_init = true;
    return CC_OK;

error:
    log_destroy(&dlog->logger);
    return CC_ERROR;
}

void
debug_teardown(void)
{
    log_stderr("Tear down the %s module", DEBUG_MODULE_NAME);

    if (!debug_init) {
        log_stderr("%s was never setup", DEBUG_MODULE_NAME);
    }

    if (dlog->logger != NULL) {
        log_destroy(&dlog->logger);
    }

    if (dlog_tev != NULL) {
        timeout_event_destroy(&dlog_tev);
    }

    debug_init = false;
}

void
_log(struct debug_logger *dl, const char *file, int line, int level, const char *fmt, ...)
{
    int len, size, errno_save;
    char buf[LOG_MAX_LEN], *timestr;
    va_list args;
    struct tm *local;
    time_t t;

    if (dl == NULL || dl->logger == NULL || dl->level < level) {
        return;
    }

    errno_save = errno;
    len = 0;            /* length of output buffer */
    size = LOG_MAX_LEN; /* size of output buffer */

    t = time(NULL);
    local = localtime(&t);
    timestr = asctime(local);

    len += cc_scnprintf(buf + len, size - len, "[%.*s][%s] %s:%d ",
            strlen(timestr) - 1, timestr, level_str[level], file, line);

    va_start(args, fmt);
    len += cc_vscnprintf(buf + len, size - len, fmt, args);
    va_end(args);

    buf[len++] = '\n';

    log_write(dl->logger, buf, len);

    errno = errno_save;
}


/*
 * Hexadecimal dump in the canonical hex + ascii display
 * See -C option in man hexdump
 */
void
_log_hexdump(struct debug_logger *dl, int level, char *data, int datalen)
{
    char buf[8 * LOG_MAX_LEN];
    int i, off, len, size, errno_save;

    if (dl->logger == NULL || dl->level < level) {
        return;
    }

    /* log hexdump */
    errno_save = errno;
    off = 0;                  /* data offset */
    len = 0;                  /* length of output buffer */
    size = 8 * LOG_MAX_LEN;   /* size of output buffer */

    while (datalen != 0 && (len < size - 1)) {
        char *save;
        unsigned char c;
        int savelen;

        len += cc_scnprintf(buf + len, size - len, "%08x  ", off);

        save = data;
        savelen = datalen;

        for (i = 0; datalen != 0 && i < 16; data++, datalen--, i++) {
            c = (unsigned char)(*data);
            len += cc_scnprintf(buf + len, size - len, "%02x%s", c,
                    (i == 7) ? "  " : " ");
        }
        for ( ; i < 16; i++) {
            len += cc_scnprintf(buf + len, size - len, "  %s",
                    (i == 7) ? "  " : " ");
        }

        data = save;
        datalen = savelen;

        len += cc_scnprintf(buf + len, size - len, "  |");

        for (i = 0; datalen != 0 && i < 16; data++, datalen--, i++) {
            c = (unsigned char)(isprint(*data) ? *data : '.');
            len += cc_scnprintf(buf + len, size - len, "%c", c);
        }
        len += cc_scnprintf(buf + len, size - len, "|\n");

        off += 16;
    }

    log_write(dl->logger, buf, len);

    errno = errno_save;
}

