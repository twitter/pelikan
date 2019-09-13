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

#include <channel/cc_tcp.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mm.h>
#include <cc_pool.h>
#include <cc_util.h>
#include <cc_event.h>

#include <errno.h>
#include <fcntl.h>
#include <inttypes.h>
#include <netinet/ip.h>
#include <netinet/tcp.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <sys/uio.h>

#define TCP_MODULE_NAME "ccommon::tcp"

FREEPOOL(tcp_conn_pool, cq, tcp_conn);
static struct tcp_conn_pool cp;

static bool tcp_init = false;
static bool cp_init = false;
static tcp_metrics_st *tcp_metrics = NULL;
static int max_backlog = TCP_BACKLOG;

void
tcp_conn_reset(struct tcp_conn *c)
{
    STAILQ_NEXT(c, next) = NULL;
    c->free = false;

    c->level = CHANNEL_INVALID;
    c->sd = 0;

    c->recv_nbyte = 0;
    c->send_nbyte = 0;

    c->state = CHANNEL_UNKNOWN;
    c->flags = 0;

    c->err = 0;
}

struct tcp_conn *
tcp_conn_create(void)
{
    struct tcp_conn *c = (struct tcp_conn *)cc_alloc(sizeof(struct tcp_conn));

    if (c == NULL) {
        log_info("connection creation failed due to OOM");
        INCR(tcp_metrics, tcp_conn_create_ex);

        return NULL;
    }

    tcp_conn_reset(c);
    INCR(tcp_metrics, tcp_conn_create);
    INCR(tcp_metrics, tcp_conn_curr);

    log_verb("created tcp_conn %p", c);

    return c;
}

void
tcp_conn_destroy(struct tcp_conn **conn)
{
    struct tcp_conn *c = *conn;

    if (c == NULL) {
        return;
    }

    log_verb("destroy tcp_conn %p", c);

    cc_free(c);
    *conn = NULL;
    INCR(tcp_metrics, tcp_conn_destroy);
    DECR(tcp_metrics, tcp_conn_curr);
}

static void
tcp_conn_pool_destroy(void)
{
    struct tcp_conn *c, *tc;

    if (!cp_init) {
        log_warn("tcp_conn pool was never created, ignore");

        return;
    }

    log_info("destroying tcp_conn pool: free %"PRIu32, cp.nfree);

    FREEPOOL_DESTROY(c, tc, &cp, next, tcp_conn_destroy);
    cp_init = false;
}

static void
tcp_conn_pool_create(uint32_t max)
{
    struct tcp_conn *c;

    if (cp_init) {
        log_warn("tcp_conn pool has already been created, re-creating");

        tcp_conn_pool_destroy();
    }

    log_info("creating tcp_conn pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&cp, max);
    cp_init = true;

    /* preallocating, see notes in buffer/cc_buf.c */
    FREEPOOL_PREALLOC(c, &cp, max, next, tcp_conn_create);
    if (cp.nfree < max) {
        log_crit("cannot preallocate tcp_conn pool due to OOM, abort");
        exit(EXIT_FAILURE);
    }
}

struct tcp_conn *
tcp_conn_borrow(void)
{
    struct tcp_conn *c;

    FREEPOOL_BORROW(c, &cp, next, tcp_conn_create);

    if (c == NULL) {
        log_debug("borrow tcp_conn failed: OOM or over limit");
        INCR(tcp_metrics, tcp_conn_borrow_ex);

        return NULL;
    }

    tcp_conn_reset(c);
    INCR(tcp_metrics, tcp_conn_borrow);
    INCR(tcp_metrics, tcp_conn_active);

    log_verb("borrow tcp_conn %p", c);

    return c;
}

void
tcp_conn_return(struct tcp_conn **c)
{
    if (c == NULL || *c == NULL || (*c)->free) {
        return;
    }

    log_verb("return tcp_conn %p", *c);

    (*c)->free = true;
    FREEPOOL_RETURN(*c, &cp, next);

    *c = NULL;
    INCR(tcp_metrics, tcp_conn_return);
    DECR(tcp_metrics, tcp_conn_active);
}

bool
tcp_connect(struct addrinfo *ai, struct tcp_conn *c)
{
    int ret;

    ASSERT(c != NULL);

    c->sd = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
    INCR(tcp_metrics, tcp_connect);
    if (c->sd < 0) {
	log_error("socket create for tcp_conn %p failed: %s", c, strerror(errno));

        goto error;
    }

    ret = tcp_set_tcpnodelay(c->sd);
    if (ret < 0) {
        log_error("set tcpnodelay on c %p sd %d failed: %s", c, c->sd,
                strerror(errno));

        goto error;
    }

    ret = connect(c->sd, ai->ai_addr, ai->ai_addrlen);
    if (ret < 0) {
        if (errno != EINPROGRESS) {
            log_error("connect on c %p sd %d failed: %s", c, c->sd,
                strerror(errno));

            goto error;
        }

        c->state = CHANNEL_OPEN;
        /* TODO(yao): if connect fails we should get an event with error mask,
         * figure out how to update metrics properly in that case.
         */
        log_info("connecting on c %p sd %d", c, c->sd);
    } else {
        c->state = CHANNEL_ESTABLISHED;
        log_info("connected on c %p sd %d", c, c->sd);
    }


    ret = tcp_set_nonblocking(c->sd);
    if (ret < 0) {
        log_error("set nonblock on c %p sd %d failed: %s", c, c->sd,
                strerror(errno));

        goto error;
    }

    return true;

error:
    c->err = errno;
    if (c->sd > 0) {
        close(c->sd);
    }
    INCR(tcp_metrics, tcp_connect_ex);

    return false;
}

bool
tcp_listen(struct addrinfo *ai, struct tcp_conn *c)
{
    int ret;
    int sd;

    c->sd = socket(ai->ai_family, ai->ai_socktype, ai->ai_protocol);
    if (c->sd < 0) {
        log_error("socket failed: %s", strerror(errno));
        goto error;
    }

    sd = c->sd;

    ret = tcp_set_reuseaddr(sd);
    if (ret < 0) {
        log_error("reuse of sd %d failed: %s", sd, strerror(errno));
        goto error;
    }

    ret = bind(sd, ai->ai_addr, ai->ai_addrlen);
    if (ret < 0) {
        log_error("bind on sd %d failed: %s", sd, strerror(errno));
        goto error;
    }

    ret = listen(sd, max_backlog);
    if (ret < 0) {
        log_error("listen on sd %d failed: %s", sd, strerror(errno));
        goto error;
    }

    ret = tcp_set_nonblocking(sd);
    if (ret != CC_OK) {
        log_error("set nonblock on sd %d failed: %s", sd, strerror(errno));
        goto error;
    }

    c->level = CHANNEL_META;
    c->state = CHANNEL_LISTEN;
    log_info("server listen setup on socket descriptor %d", c->sd);

    return true;

error:
    if (c->sd > 0) {
        tcp_close(c);
    }

    return false;
}

void
tcp_close(struct tcp_conn *c)
{
    int ret;

    if (c == NULL) {
        return;
    }

    log_info("closing tcp_conn %p sd %d", c, c->sd);

    INCR(tcp_metrics, tcp_close);
    ret = close(c->sd);
    if (ret < 0) {
        log_warn("close c %d failed, ignored: %s", c->sd, strerror(errno));
    }
}

static inline int
_tcp_accept(struct tcp_conn *sc)
{
    int sd;

    ASSERT(sc->sd >= 0);

    /* How does tcp accept work when a separate thread is used to accept new
     * connections?
     *
     * In general, we want to accept a new connection at a time (on the server
     * thread), then hand this connection over to be put on some other event
     * loop (e.g. on a worker thread's), and some additional preparation may
     * be necessary (e.g. allocating R/W buffers). This is why we break after
     * completing a single `accept' successfully.
     *
     * There are several ways `accept' could "fail", and they need to be
     * treated differently. The most common case, which isn't really a failure
     * on a non-blocking socket, is receiving EAGAIN/EWOULDBLOCK. This simply
     * means there is no new connection to accept and the function should
     * return.
     * EINTR is another common error which means the call was terminated by
     * a signal. This type of interruption is almost always transient, so
     * an immediate retry is likely to succeed.
     * The rest of the exceptions likely to occur on a SOCK_STREAM socket are
     * often due to exhaustion of some resources (e.g. fd, memory), and there
     * is no guarantee they will recover immediately. For example, to free
     * up another fd requires an existing connection to be closed. In such
     * cases, the connection in the backlog will sit there (as fully
     * established as far as TCP stack is concerned) until accept in application
     * becomes possible again, and any new connections arriving will be added
     * to the back of the queue until it's full, at which point the client
     * will receive an exception and the connect attempt will fail.
     */
    for (;;) {
#ifdef CC_ACCEPT4
        sd = accept4(sc->sd, NULL, NULL, SOCK_NONBLOCK);
#else
        sd = accept(sc->sd, NULL, NULL);
#endif /* CC_ACCEPT4 */
        if (sd < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                log_debug("accept on sd %d not ready: eagain", sc->sd);
                return -1;
            }

            if (errno == EINTR) {
                log_debug("accept on sd %d not ready: eintr", sc->sd);

                continue;
            }

            log_error("accept on sd %d failed: %s", sc->sd, strerror(errno));
            INCR(tcp_metrics, tcp_accept_ex);

            return -1;
        }

        break;
    }

    ASSERT(sd >= 0);
    return sd;
}

bool
tcp_accept(struct tcp_conn *sc, struct tcp_conn *c)
{
    int ret;
    int sd;

    sd = _tcp_accept(sc);
    INCR(tcp_metrics, tcp_accept);
    if (sd < 0) {
        return false;
    }

    c->sd = sd;
    c->level = CHANNEL_BASE;
    c->state = CHANNEL_ESTABLISHED;

#ifndef CC_ACCEPT4 /* if we have accept4, nonblock will already have been set */
    ret = tcp_set_nonblocking(sd);
    if (ret < 0) {
        log_warn("set nonblock on sd %d failed, ignored: %s", sd,
                strerror(errno));
    }
#endif

    ret = tcp_set_tcpnodelay(sd);
    if (ret < 0) {
        log_warn("set tcp nodelay on sd %d failed, ignored: %s", sd,
                 strerror(errno));
    }

    log_info("accepted c %d on sd %d", c->sd, sc->sd);

    return true;
}


/*
 * due to lack of a direct rejection API in POSIX, tcp_reject accepts the
 * frontmost connection and immediately closes it
 */
void
tcp_reject(struct tcp_conn *sc)
{
    int ret;
    int sd;

    INCR(tcp_metrics, tcp_reject);
    sd = _tcp_accept(sc);
    if (sd < 0) {
        INCR(tcp_metrics, tcp_reject_ex);
        return;
    }

    ret = close(sd);
    if (ret < 0) {
        INCR(tcp_metrics, tcp_reject_ex);
        log_warn("close c %d failed, ignored: %s", sd, strerror(errno));
    }
}

/*
 * due to lack of a direct rejection API in POSIX, tcp_reject_all accepts
 * connections ready on the listening socket, and immediately closes them.
 * It does so until there are no more pending connections.
 */
void
tcp_reject_all(struct tcp_conn *sc)
{
    int ret;
    int sd;

    for (;;) {
        sd = accept(sc->sd, NULL, NULL);
        if (sd < 0) {
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                log_debug("sd %d has no more outstanding connections", sc->sd);
                return;
            }

            if (errno == EINTR) {
                log_debug("accept on sd %d not ready: eintr", sc->sd);

                continue;
            }

            log_error("accept on sd %d failed: %s", sc->sd, strerror(errno));
            INCR(tcp_metrics, tcp_reject_ex);

            return;
        }

        ret = close(sd);
        if (ret < 0) {
            INCR(tcp_metrics, tcp_reject_ex);
            log_warn("close c %d failed, ignored: %s", sd, strerror(errno));
        }

        INCR(tcp_metrics, tcp_reject);
    }
}

int
tcp_set_blocking(int sd)
{
    int flags;

    flags = fcntl(sd, F_GETFL, 0);
    if (flags < 0) {
        return flags;
    }

    return fcntl(sd, F_SETFL, flags & ~O_NONBLOCK);
}

int
tcp_set_nonblocking(int sd)
{
    int flags;

    flags = fcntl(sd, F_GETFL, 0);
    if (flags < 0) {
        return flags;
    }

    return fcntl(sd, F_SETFL, flags | O_NONBLOCK);
}

int
tcp_set_reuseaddr(int sd)
{
    int reuse;
    socklen_t len;

    reuse = 1;
    len = sizeof(reuse);

    return setsockopt(sd, SOL_SOCKET, SO_REUSEADDR, &reuse, len);
}

/*
 * Disable Nagle algorithm on TCP socket.
 *
 * This option helps to minimize transmit latency by disabling coalescing
 * of data to fill up a TCP segment inside the kernel. Sockets with this
 * option must use readv() or writev() to do data transfer in bulk and
 * hence avoid the overhead of small packets.
 */
int
tcp_set_tcpnodelay(int sd)
{
    int nodelay;
    socklen_t len;

    nodelay = 1;
    len = sizeof(nodelay);

    return setsockopt(sd, IPPROTO_TCP, TCP_NODELAY, &nodelay, len);
}

/*TODO set keepalive paramters (probes, intvl, wait) */
int
tcp_set_keepalive(int sd)
{
    int keepalive;
    socklen_t len;

    keepalive = 1;
    len = sizeof(keepalive);

    return setsockopt(sd, SOL_SOCKET, SO_KEEPALIVE, &keepalive, len);
}

int
tcp_set_linger(int sd, int timeout)
{
    struct linger linger;
    socklen_t len;

    linger.l_onoff = 1;
    linger.l_linger = timeout;

    len = sizeof(linger);

    return setsockopt(sd, SOL_SOCKET, SO_LINGER, &linger, len);
}

int
tcp_unset_linger(int sd)
{
    struct linger linger;
    socklen_t len;

    linger.l_onoff = 0;
    linger.l_linger = 0;

    len = sizeof(linger);

    return setsockopt(sd, SOL_SOCKET, SO_LINGER, &linger, len);
}

int
tcp_set_sndbuf(int sd, int size)
{
    socklen_t len;

    len = sizeof(size);

    return setsockopt(sd, SOL_SOCKET, SO_SNDBUF, &size, len);
}

int
tcp_set_rcvbuf(int sd, int size)
{
    socklen_t len;

    len = sizeof(size);

    return setsockopt(sd, SOL_SOCKET, SO_RCVBUF, &size, len);
}

int
tcp_get_sndbuf(int sd)
{
    int status, size;
    socklen_t len;

    size = 0;
    len = sizeof(size);

    status = getsockopt(sd, SOL_SOCKET, SO_SNDBUF, &size, &len);
    if (status < 0) {
        return status;
    }

    return size;
}

int
tcp_get_rcvbuf(int sd)
{
    int status, size;
    socklen_t len;

    size = 0;
    len = sizeof(size);

    status = getsockopt(sd, SOL_SOCKET, SO_RCVBUF, &size, &len);
    if (status < 0) {
        return status;
    }

    return size;
}

void
tcp_maximize_sndbuf(int sd)
{
    int status, min, max, avg;

    /* start with the default size */
    min = tcp_get_sndbuf(sd);
    if (min < 0) {
        return;
    }

    /* binary-search for the real maximum */
    max = 256 * MiB;

    while (min <= max) {
        avg = (min + max) / 2;
        status = tcp_set_sndbuf(sd, avg);
        if (status != 0) {
            max = avg - 1;
        } else {
            min = avg + 1;
        }
    }
}

int
tcp_get_soerror(int sd)
{
    int status, err;
    socklen_t len;

    err = 0;
    len = sizeof(err);

    status = getsockopt(sd, SOL_SOCKET, SO_ERROR, &err, &len);
    if (status == 0) {
        errno = err;
    }

    return status;
}


/*
 * try reading nbyte bytes from tcp_conn and place the data in buf
 * EINTR is continued, EAGAIN is explicitly flagged in return, other errors are
 * returned as a generic error/failure.
 */
ssize_t
tcp_recv(struct tcp_conn *c, void *buf, size_t nbyte)
{
    ssize_t n;

    ASSERT(buf != NULL);
    ASSERT(nbyte > 0);

    log_verb("recv on sd %d, capacity %zu bytes", c->sd, nbyte);

    for (;;) {
        n = read(c->sd, buf, nbyte);
        INCR(tcp_metrics, tcp_recv);

        log_verb("read on sd %d %zd of %zu", c->sd, n, nbyte);

        if (n > 0) {
            log_verb("%zu bytes recv'd on sd %d", n, c->sd);
            c->recv_nbyte += (size_t)n;
            INCR_N(tcp_metrics, tcp_recv_byte, n);
            return n;
        }

        if (n == 0) {
            c->state = CHANNEL_TERM;
            log_debug("eof recv'd on sd %d, total: rb %zu sb %zu", c->sd,
                      c->recv_nbyte, c->send_nbyte);
            return n;
        }

        /* n < 0 */
        INCR(tcp_metrics, tcp_recv_ex);
        if (errno == EINTR) {
            log_debug("recv on sd %d not ready - EINTR", c->sd);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            log_debug("recv on sd %d not ready - EAGAIN", c->sd);
            return CC_EAGAIN;
        } else {
            c->err = errno;
            log_error("recv on sd %d failed: %s", c->sd, strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

/*
 * vector version of tcp_recv, using readv to read into a mbuf array
 */
ssize_t
tcp_recvv(struct tcp_conn *c, struct array *bufv, size_t nbyte)
{
    /* TODO(yao): this is almost identical with tcp_recv except for the call
     * to readv. Consolidate the two?
     */
    ssize_t n;

    ASSERT(array_nelem(bufv) > 0);
    ASSERT(nbyte != 0);

    log_verb("recvv on sd %d, total %zu bytes", c->sd, nbyte);

    for (;;) {
        n = readv(c->sd, (const struct iovec *)bufv->data, bufv->nelem);
        INCR(tcp_metrics, tcp_recv);

        log_verb("recvv on sd %d %zd of %zu in %"PRIu32" buffers",
                  c->sd, n, nbyte, bufv->nelem);

        if (n > 0) {
            c->recv_nbyte += (size_t)n;
            INCR_N(tcp_metrics, tcp_recv, n);
            return n;
        }

        if (n == 0) {
            log_warn("recvv on sd %d returned zero", c->sd);

            return 0;
        }

        /* n < 0 */
        INCR(tcp_metrics, tcp_recv_ex);
        if (errno == EINTR) {
            log_verb("recvv on sd %d not ready - eintr", c->sd);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {

            log_verb("recvv on sd %d not ready - eagain", c->sd);
            return CC_EAGAIN;
        } else {

            c->err = errno;
            log_error("recvv on sd %d failed: %s", c->sd, strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

/*
 * try writing nbyte to tcp_conn and store the data in buf
 * EINTR is continued, EAGAIN is explicitly flagged in return, other errors are
 * returned as a generic error/failure.
 */
ssize_t
tcp_send(struct tcp_conn *c, void *buf, size_t nbyte)
{
    ssize_t n;

    ASSERT(buf != NULL);
    ASSERT(nbyte > 0);

    log_verb("send on sd %d, total %zu bytes", c->sd, nbyte);

    for (;;) {
        n = write(c->sd, buf, nbyte);
        INCR(tcp_metrics, tcp_send);

        log_verb("write on sd %d %zd of %zu", c->sd, n, nbyte);

        if (n > 0) {
            INCR_N(tcp_metrics, tcp_send_byte, n);
            c->send_nbyte += (size_t)n;
            return n;
        }

        if (n == 0) {
            log_warn("write on sd %d returned zero", c->sd);
            return 0;
        }

        /* n < 0 */
        INCR(tcp_metrics, tcp_send_ex);
        if (errno == EINTR) {
            log_verb("write on sd %d not ready - EINTR", c->sd);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            log_verb("write on sd %d not ready - EAGAIN", c->sd);
            return CC_EAGAIN;
        } else {
            c->err = errno;
            log_error("write on sd %d failed: %s", c->sd, strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

/*
 * vector version of tcp_send, using writev to send an array of bufs
 */
ssize_t
tcp_sendv(struct tcp_conn *c, struct array *bufv, size_t nbyte)
{
    /* TODO(yao): this is almost identical with tcp_send except for the call
     * to writev. Consolidate the two? Revisit these functions when we build
     * more concrete backend systems.
     */
    ssize_t n;

    ASSERT(array_nelem(bufv) > 0);
    ASSERT(nbyte != 0);

    log_verb("sendv on sd %d, total %zu bytes", c->sd, nbyte);

    for (;;) {
        n = writev(c->sd, (const struct iovec *)bufv->data, bufv->nelem);
        INCR(tcp_metrics, tcp_send_ex);

        log_verb("writev on sd %d %zd of %zu in %"PRIu32" buffers",
                  c->sd, n, nbyte, bufv->nelem);

        if (n > 0) {
            c->send_nbyte += (size_t)n;
            INCR_N(tcp_metrics, tcp_send_byte, n);
            return n;
        }

        if (n == 0) {
            log_warn("sendv on sd %d returned zero", c->sd);
            return 0;
        }

        /* n < 0, error */
        INCR(tcp_metrics, tcp_send_ex);
        if (errno == EINTR) {
            log_verb("sendv on sd %d not ready - eintr", c->sd);
            continue;
        } else if (errno == EAGAIN || errno == EWOULDBLOCK) {
            log_verb("sendv on sd %d not ready - eagain", c->sd);
            return CC_EAGAIN;
        } else {
            c->err = errno;
            log_error("sendv on sd %d failed: %s", c->sd, strerror(errno));
            return CC_ERROR;
        }
    }

    NOT_REACHED();

    return CC_ERROR;
}

void
tcp_setup(tcp_options_st *options, tcp_metrics_st *metrics)
{
    uint32_t max = TCP_POOLSIZE;

    log_info("set up the %s module", TCP_MODULE_NAME);

    if (tcp_init) {
        log_warn("%s has already been setup, overwrite", TCP_MODULE_NAME);
    }

    tcp_metrics = metrics;

    if (options != NULL) {
        max_backlog = option_uint(&options->tcp_backlog);
        max = option_uint(&options->tcp_poolsize);
    }
    tcp_conn_pool_create(max);

    channel_sigpipe_ignore(); /* does it ever fail? */
    tcp_init = true;
}

void
tcp_teardown(void)
{
    log_info("tear down the %s module", TCP_MODULE_NAME);

    if (!tcp_init) {
        log_warn("%s has never been setup", TCP_MODULE_NAME);
    }

    tcp_conn_pool_destroy();
    tcp_metrics = NULL;

    tcp_init = false;
}
