/*
 * ccommon cache common library.
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

#include <cc_log.h>

#include <cc_mm.h>
#include <cc_print.h>
#include <cc_rbuf.h>
#include <cc_util.h>

#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>

#define LOG_MODULE_NAME "ccommon::log"

static log_metrics_st *log_metrics = NULL;
static bool log_init = false;

void
log_setup(log_metrics_st *metrics)
{
    log_stderr("set up the %s module", LOG_MODULE_NAME);

    log_metrics = metrics;
    if (metrics != NULL) {
        LOG_METRIC_INIT(log_metrics);
    }

    if (log_init) {
        log_stderr("%s has already been setup, overwrite", LOG_MODULE_NAME);
    }
    log_init = true;
}

void
log_teardown(void)
{
    log_stderr("tear down the %s module", LOG_MODULE_NAME);

    if (!log_init) {
        log_stderr("%s has never been setup", LOG_MODULE_NAME);
    }

    log_metrics = NULL;
    log_init = false;
}

struct logger *
log_create(char *filename, uint32_t buf_cap)
{
    struct logger *logger;

    log_stderr("create logger with filename %s cap %u", filename, buf_cap);

    logger = cc_alloc(sizeof(struct logger));

    if (logger == NULL) {
        log_stderr("Could not create logger due to OOM");
        INCR(log_metrics, log_create_ex);
        return NULL;
    }

    logger->name = filename;

    if (buf_cap > 0) {
        logger->buf = rbuf_create(buf_cap);

        if (logger->buf == NULL) {
            cc_free(logger);
            log_stderr("Could not create logger - buffer not allocated due to OOM");
            INCR(log_metrics, log_create_ex);
            return NULL;
        }
    } else {
        logger->buf = NULL;
    }

    if (filename != NULL) {
        logger->fd = open(filename, O_WRONLY | O_APPEND | O_CREAT, 0644);
        if (logger->fd < 0) {
            cc_free(logger);
            log_stderr("Could not create logger - cannot open file");
            INCR(log_metrics, log_open_ex);
            INCR(log_metrics, log_create_ex);
            return NULL;
        } else {
            INCR(log_metrics, log_open);
        }
    } else {
        logger->fd = STDERR_FILENO;
    }

    INCR(log_metrics, log_create);
    INCR(log_metrics, log_curr);

    return logger;
}

void
log_destroy(struct logger **l)
{
    struct logger *logger = *l;

    if (logger == NULL) {
        return;
    }

    /* flush first in case there's data left in the buffer */
    log_flush(logger);

    if (logger->fd >= 0 && logger->fd != STDERR_FILENO
        && logger->fd != STDOUT_FILENO) {
        close(logger->fd);
    }

    rbuf_destroy(&logger->buf);

    cc_free(logger);
    *l = NULL;

    INCR(log_metrics, log_destroy);
    DECR(log_metrics, log_curr);
}

rstatus_i
log_reopen(struct logger *logger)
{
    if (logger->fd != STDERR_FILENO && logger->fd != STDOUT_FILENO) {
        close(logger->fd);
        logger->fd = open(logger->name, O_WRONLY | O_APPEND | O_CREAT, 0644);
        if (logger->fd < 0) {
            log_stderr("reopening log file '%s' failed, ignored: %s", logger->name,
                       strerror(errno));
            INCR(log_metrics, log_open_ex);
            return CC_ERROR;
        }
    }

    INCR(log_metrics, log_open);

    return CC_OK;
}

bool
log_write(struct logger *logger, char *buf, uint32_t len)
{
    if (logger->buf != NULL) {
        if (rbuf_wcap(logger->buf) >= len) {
            rbuf_write(logger->buf, buf, len);
            INCR(log_metrics, log_write);
            INCR_N(log_metrics, log_write_byte, len);
        } else {
            INCR(log_metrics, log_skip);
            INCR_N(log_metrics, log_skip_byte, len);
            return false;
        }
    } else {
        if (logger->fd < 0) {
            INCR(log_metrics, log_write_ex);
            return false;
        }

        if (write(logger->fd, buf, len) < (ssize_t)len) {
            INCR(log_metrics, log_write_ex);
            return false;
        }

        INCR(log_metrics, log_write);
        INCR_N(log_metrics, log_write_byte, len);
    }

    return true;
}

void
_log_fd(int fd, const char *fmt, ...)
{
    int len, size, errno_save;
    char buf[LOG_MAX_LEN];
    va_list args;
    ssize_t n;

    errno_save = errno;
    len = 0;            /* length of output */
    size = LOG_MAX_LEN; /* size of output buffer */

    va_start(args, fmt);
    len += cc_vscnprintf(buf, size, fmt, args);
    va_end(args);

    buf[len++] = '\n';

    n = write(fd, buf, len);

    if (n < 0) {
        INCR(log_metrics, log_write_ex);
    }

    if (n < len) {
        INCR(log_metrics, log_skip);
        INCR_N(log_metrics, log_skip_byte, len - n);
    } else {
        INCR(log_metrics, log_write);
        INCR_N(log_metrics, log_write_byte, len);
    }

    errno = errno_save;
}


/* read from rbuf to the fd. attempts to empty the buffer. */
static ssize_t
_rbuf_flush(struct rbuf *buf, int fd)
{
    uint32_t capacity;
    ssize_t ret;
    uint32_t rpos, wpos;
    rpos = get_rpos(buf);
    wpos = get_wpos(buf);

    if (wpos < rpos) {
        /* write until end, then wrap around */
        capacity = buf->cap - rpos + 1;
        ret = write(fd, buf->data + rpos, capacity);

        if (ret > 0) {
            rpos += ret;
        }

        if (ret == capacity) {
            /* more can be written, read from beginning of buf */
            ssize_t remaining_bytes;

            capacity = wpos;
            remaining_bytes = write(fd, buf->data, capacity);

            if (remaining_bytes >= 0) {
                rpos = remaining_bytes;
                ret += remaining_bytes;
            }
        }
    } else {
        /* no wrap around */
        capacity = wpos - rpos;
        ret = write(fd, buf->data + rpos, capacity);

        if (ret > 0) {
            rpos += ret;
        }
    }

    set_rpos(buf, rpos);

    return ret;
}

size_t
log_flush(struct logger *logger)
{
    ssize_t n;
    size_t buf_len;

    if (logger->buf == NULL) {
        return;
    }

    if (logger->fd < 0) {
        log_stderr("Cannot flush logger %p; invalid file descriptor", logger);
        INCR(log_metrics, log_flush_ex);
        return;
    }

    buf_len = rbuf_rcap(logger->buf);
    n = _rbuf_flush(logger->buf, logger->fd);

    if (n < (ssize_t)buf_len) {
        INCR(log_metrics, log_flush_ex);
    } else {
        INCR(log_metrics, log_flush);
    }

    return n > 0 ? n : 0;
}
