/*
 * ccommon - a cache common library.
 * Copyright (C) 2015 Twitter, Inc.
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

#include <channel/cc_pipe.h>

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>
#include <channel/cc_channel.h>
#include <channel/cc_pipe.h>

#include <errno.h>
#include <fcntl.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

#define PIPE_MODULE_NAME "ccommon::pipe"

FREEPOOL(pipe_conn_pool, cq, pipe_conn);
static struct pipe_conn_pool cp;
static bool cp_init = false;

static bool pipe_init = false;
static pipe_metrics_st *pipe_metrics = NULL;

void
pipe_setup(pipe_metrics_st *metrics)
{
    log_info("set up the %s module", PIPE_MODULE_NAME);
    if (pipe_init) {
        log_warn("%s has already been setup, overwrite", PIPE_MODULE_NAME);
    }

    pipe_metrics = metrics;
    if (metrics != NULL) {
        PIPE_METRIC_INIT(pipe_metrics);
    }

    channel_sigpipe_ignore(); /* does it ever fail */
    pipe_init = true;
}

void
pipe_teardown(void)
{
    log_info("tear down the %s module", PIPE_MODULE_NAME);

    if (!pipe_init) {
        log_warn("%s has never been setup", PIPE_MODULE_NAME);
    }

    pipe_metrics = NULL;
    pipe_init = false;
}

struct pipe_conn *
pipe_conn_create(void)
{
    struct pipe_conn *c = (struct pipe_conn *)cc_alloc(sizeof(struct pipe_conn));

    if (c == NULL) {
        log_info("pipe connection creation failed due to OOM");
        INCR(pipe_metrics, pipe_conn_create_ex);
        return NULL;
    }

    log_verb("created pipe conn %p", c);

    pipe_conn_reset(c);

    INCR(pipe_metrics, pipe_conn_create);
    INCR(pipe_metrics, pipe_conn_curr);

    return c;
}

void
pipe_conn_destroy(struct pipe_conn **c)
{
    if (c == NULL || *c == NULL) {
        return;
    }

    log_verb("destroy conn %p", *c);

    cc_free(*c);
    c = NULL;

    INCR(pipe_metrics, pipe_conn_destroy);
    DECR(pipe_metrics, pipe_conn_curr);
}

void
pipe_conn_reset(struct pipe_conn *c)
{
    STAILQ_NEXT(c, next) = NULL;
    c->free = false;

    c->fd[0] = c->fd[1] = 0;

    c->recv_nbyte = 0;
    c->send_nbyte = 0;

    c->state = CHANNEL_TERM;
    c->flags = 0;

    c->err = 0;
}

void
pipe_conn_pool_create(uint32_t max)
{
    if (!cp_init) {
        uint32_t i;
        struct pipe_conn *c;

        log_info("creating conn pool: max %"PRIu32, max);

        FREEPOOL_CREATE(&cp, max);
        cp_init = true;

        /* preallocate */
        if (max == 0) {
            return;
        }

        for (i = 0; i < max; ++i) {
            c = pipe_conn_create();
            if (c == NULL) {
                log_crit("cannot preallocate pipe conn pool, OOM");
                exit(EXIT_FAILURE);
            }
            c->free = true;
            FREEPOOL_RETURN(&cp, c, next);
        }
    } else {
        log_warn("conn pool has already been created, ignore");
    }
}

void
pipe_conn_pool_destroy(void)
{
    struct pipe_conn *c, *tc;

    if (cp_init) {
        log_info("destroying pipe conn pool: free %"PRIu32, cp.nfree);

        FREEPOOL_DESTROY(c, tc, &cp, next, pipe_conn_destroy);
        cp_init = false;
    } else {
        log_warn("pipe conn pool was never created, ignore");
    }
}

struct pipe_conn *
pipe_conn_borrow(void)
{
    struct pipe_conn *c;

    FREEPOOL_BORROW(c, &cp, next, pipe_conn_create);

    if (c == NULL) {
        INCR(pipe_metrics, pipe_conn_borrow_ex);
        log_debug("borrow pipe conn failed: OOM or over limit");
        return NULL;
    }

    pipe_conn_reset(c);
    INCR(pipe_metrics, pipe_conn_borrow);
    INCR(pipe_metrics, pipe_conn_active);

    log_verb("borrow conn %p", c);

    return c;
}

void
pipe_conn_return(struct pipe_conn **c)
{
    if (c == NULL || *c == NULL || (*c)->free) {
        return;
    }

    log_verb("return conn %p", *c);

    (*c)->free = true;
    FREEPOOL_RETURN(&cp, *c, next);

    *c = NULL;
    INCR(pipe_metrics, pipe_conn_return);
    DECR(pipe_metrics, pipe_conn_active);
}

bool
pipe_open(void *addr, struct pipe_conn *c)
{
    int status;

    ASSERT(c != NULL);

    status = pipe(c->fd);
    if (status) {
        log_error("pipe() for conn %p failed: %s", c, strerror(errno));
        goto error;
    }

    c->state = CHANNEL_LISTEN;
    INCR(pipe_metrics, pipe_open);
    return true;

error:
    c->err = errno;

    INCR(pipe_metrics, pipe_open_ex);

    return false;
}

void
pipe_close(struct pipe_conn *c)
{
    if (c == NULL) {
        return;
    }

    log_info("closing pipe conn %p fd %d and %d", c, c->fd[0], c->fd[1]);

    if (c->fd[0] >= 0) {
        close(c->fd[0]);
    }

    if (c->fd[1] >= 0) {
        close(c->fd[1]);
    }

    INCR(pipe_metrics, pipe_close);
}

ssize_t
pipe_recv(struct pipe_conn *c, void *buf, size_t nbyte)
{
    ssize_t n;

    ASSERT(c != NULL);
    ASSERT(buf != NULL);
    ASSERT(nbyte > 0);

    log_verb("recv on pipe fd %d, capacity %zu bytes", c->fd[0], nbyte);

    /* TODO(kyang): see if this can be refactored to remove duplicate code
       w/ cc_tcp */
    for (;;) {
        n = read(c->fd[0], buf, nbyte);
        INCR(pipe_metrics, pipe_recv);

        log_verb("read on fd %d %zd of %zu", c->fd[0], n, nbyte);

        if (n > 0) {
            log_verb("%zu bytes recv'd on fd %d", n, c->fd[0]);
            c->recv_nbyte += (size_t)n;
            INCR_N(pipe_metrics, pipe_recv_byte, n);
            return n;
        }

        if (n == 0) {
            log_debug("eof recv'd on fd %d, total: rb %zu sb %zu", c->fd[0],
                      c->recv_nbyte, c->send_nbyte);
            return n;
        }

        /* n < 0 */
        INCR(pipe_metrics, pipe_recv_ex);
        if (errno == EINTR) {
            log_debug("recv on fd %d not ready - EINTR", c->fd[0]);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            log_debug("recv on fd %d not ready - EAGAIN", c->fd[0]);
            return CC_EAGAIN;
        } else {
            c->err = errno;
            log_error("recv on fd %d failed: %s", c->fd[0], strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

ssize_t pipe_send(struct pipe_conn *c, void *buf, size_t nbyte)
{
    ssize_t n;

    ASSERT(c != NULL);
    ASSERT(buf != NULL);
    ASSERT(nbyte > 0);

    log_verb("send on fd %d, total %zu bytes", c->fd[1], nbyte);

    for (;;) {
        n = write(c->fd[1], buf, nbyte);
        INCR(pipe_metrics, pipe_send);

        log_verb("write on fd %d %zd of %zu", c->fd, n, nbyte);

        if (n > 0) {
            log_verb("%zu bytes sent on fd %d", n, c->fd[1]);
            c->send_nbyte += (size_t)n;
            INCR_N(pipe_metrics, pipe_send_byte, n);
            return n;
        }

        if (n == 0) {
            log_warn("sendv on fd %d returned zero", c->fd[1]);
            return 0;
        }

        /* n < 0 */
        INCR(pipe_metrics, pipe_send_ex);
        if (errno == EINTR) {
            log_verb("send on fd %d not ready - EINTR", c->fd[1]);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            log_verb("send on fd %d not ready - EAGAIN", c->fd[1]);
            return CC_EAGAIN;
        } else {
            c->err = errno;
            log_error("sendv on fd %d failed: %s", c->fd[1], strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

static void
_pipe_set_blocking(int fd)
{
    int flags = fcntl(fd, F_GETFL, 0);

    if (flags < 0) {
        log_error("pipe set blocking flag resulted in error");
        INCR(pipe_metrics, pipe_flag_ex);
    } else {
        fcntl(fd, F_SETFL, flags & ~O_NONBLOCK);
    }
}

void
pipe_set_blocking(struct pipe_conn *c)
{
    ASSERT(c != NULL);
    _pipe_set_blocking(pipe_read_id(c));
    _pipe_set_blocking(pipe_write_id(c));
}

static void
_pipe_set_nonblocking(int fd)
{
    int flags = fcntl(fd, F_GETFL, 0);

    if (flags < 0) {
        log_error("pipe set nonblocking flag resulted in error");
        INCR(pipe_metrics, pipe_flag_ex);
    } else {
        fcntl(fd, F_SETFL, flags | O_NONBLOCK);
    }
}

void
pipe_set_nonblocking(struct pipe_conn *c)
{
    ASSERT(c != NULL);
    _pipe_set_nonblocking(pipe_read_id(c));
    _pipe_set_nonblocking(pipe_write_id(c));
}
