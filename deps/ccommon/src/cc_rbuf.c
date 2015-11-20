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

#include <cc_rbuf.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

#define RBUF_MODULE_NAME "ccommon::rbuf"

struct rbuf {
    uint32_t     rpos;          /* read offset */
    uint32_t     wpos;          /* write offset */
    uint32_t     cap;           /* # bytes allocated for data */
    uint8_t      data[1];       /* beginning of buffer */
};

#define RBUF_HDR_SIZE   offsetof(struct rbuf, data)

static rbuf_metrics_st *rbuf_metrics = NULL;
static bool rbuf_init = false;

void
rbuf_setup(rbuf_metrics_st *metrics)
{
    log_info("set up the %s module", RBUF_MODULE_NAME);

    rbuf_metrics = metrics;
    if (metrics != NULL) {
        RBUF_METRIC_INIT(rbuf_metrics);
    }

    if (rbuf_init) {
        log_warn("%s has already been setup, overwrite", RBUF_MODULE_NAME);
    }

    rbuf_init = true;
}

void
rbuf_teardown(void)
{
    log_info("tear down the %s module", RBUF_MODULE_NAME);

    if (!rbuf_init) {
        log_warn("%s has never been setup", RBUF_MODULE_NAME);
    }

    rbuf_metrics = NULL;
    rbuf_init = false;
}

struct rbuf *
rbuf_create(uint32_t cap)
{
    struct rbuf *buf;

    log_verb("Create ring buffer with capacity %zu", cap);

    buf = cc_alloc(RBUF_HDR_SIZE + cap + 1);

    if (buf == NULL) {
        log_error("Could not allocate rbuf with capacity %zu due to OOM", cap);
        INCR(rbuf_metrics, rbuf_create_ex);
        return NULL;
    }

    buf->wpos = buf->rpos = 0;
    buf->cap = cap;

    INCR(rbuf_metrics, rbuf_create);
    INCR(rbuf_metrics, rbuf_curr);
    INCR_N(rbuf_metrics, rbuf_byte, RBUF_HDR_SIZE + cap + 1);

    return buf;
}

void
rbuf_destroy(struct rbuf *buf)
{
    log_verb("Destroy ring buffer %p", buf);

    if (buf != NULL) {
#if defined CC_STATS && CC_STATS == 1
        uint32_t cap = buf->cap;
#endif

        cc_free(buf);
        INCR(rbuf_metrics, rbuf_destroy);
        DECR(rbuf_metrics, rbuf_curr);
        DECR_N(rbuf_metrics, rbuf_byte, RBUF_HDR_SIZE + cap + 1);
    }
}

static inline uint32_t
_get_rpos(struct rbuf *buf)
{
    return __atomic_load_n(&(buf->rpos), __ATOMIC_RELAXED);
}

static inline uint32_t
_get_wpos(struct rbuf *buf)
{
    return __atomic_load_n(&(buf->wpos), __ATOMIC_RELAXED);
}

static inline void
_set_rpos(struct rbuf *buf, uint32_t rpos)
{
    __atomic_store_n(&(buf->rpos), rpos, __ATOMIC_RELAXED);
}

static inline void
_set_wpos(struct rbuf *buf, uint32_t wpos)
{
    __atomic_store_n(&(buf->wpos), wpos, __ATOMIC_RELAXED);
}

/**
 * RBUF READ/WRITE CAPACITY:
 *
 * Cases:
 *
 *  1. wpos < rpos
 *     wcap = rpos - wpos - 1
 *
 *  +--------------------------+
 *  |   |||||||||||||||||||    |
 *  +---|------------------|---+
 *      ^                  ^
 *      |                  |
 *      \                  \
 *      wpos               rpos
 *
 *     rcap = cap + wpos - rpos + 1
 *
 *  +--------------------------+
 *  |||||                  |||||
 *  +---|------------------|---+
 *      ^                  ^
 *      |                  |
 *      \                  \
 *      wpos               rpos
 *
 *
 *  2. wpos >= rpos
 *     wcap = cap - wpos + rpos
 *
 *  +--------------------------+
 *  ||||                   |||||
 *  +---|------------------|---+
 *      ^                  ^
 *      |                  |
 *      \                  \
 *      rpos               wpos
 *
 *     rcap = wpos - rpos
 *
 *  +--------------------------+
 *  |   ||||||||||||||||||||   |
 *  +---|------------------|---+
 *      ^                  ^
 *      |                  |
 *      \                  \
 *      rpos               wpos
 *
 */

size_t
rbuf_rcap(struct rbuf *buf)
{
    uint32_t rpos, wpos;
    rpos = _get_rpos(buf);
    wpos = _get_wpos(buf);

    if (wpos < rpos) {
        return buf->cap + wpos - rpos + 1;
    } else {
        return wpos - rpos;
    }
}

size_t
rbuf_wcap(struct rbuf *buf)
{
    uint32_t rpos, wpos;
    rpos = _get_rpos(buf);
    wpos = _get_wpos(buf);

    if (wpos < rpos) {
        /* no wrap around */
        return rpos - wpos - 1;
    } else {
        return buf->cap - wpos + rpos;
    }
}

static inline uint32_t
_min(uint32_t first, uint32_t second)
{
    return first < second ? first : second;
}

size_t
rbuf_read(void *dst, struct rbuf *src, size_t n)
{
    size_t capacity, ret;
    uint32_t rpos, wpos;
    rpos = _get_rpos(src);
    wpos = _get_wpos(src);

    if (wpos < rpos) {
        /* write until end, then wrap around */
        capacity = src->cap - rpos + 1;
        ret = _min(capacity, n);
        cc_memcpy(dst, src->data + rpos, ret);

        if (ret < n) {
            /* start copying from beginning of srcfer */
            size_t remaining_bytes;

            capacity = wpos;
            remaining_bytes = _min(capacity, n - ret);

            cc_memcpy((uint8_t *)dst + ret, src->data, remaining_bytes);

            ret += remaining_bytes;
            rpos = remaining_bytes;
        } else {
            rpos += ret;
        }
    } else {
        /* no wrapping around */
        capacity = wpos - rpos;
        ret = _min(capacity, n);
        cc_memcpy(dst, src->data + rpos, ret);

        rpos += ret;
    }

    _set_rpos(src, rpos);

    return ret;
}

size_t
rbuf_write(struct rbuf *dst, void *src, size_t n)
{
    size_t capacity, ret;
    uint32_t rpos, wpos;
    rpos = _get_rpos(dst);
    wpos = _get_wpos(dst);

    if (wpos < rpos) {
        /* no wrapping around */
        capacity = rpos - wpos - 1;
        ret = _min(capacity, n);
        cc_memcpy(dst->data + wpos, src, ret);

        wpos += ret;
    } else {
        /* read until end, then wrap around */
        capacity = dst->cap - wpos + 1;
        ret = _min(capacity, n);

        cc_memcpy(dst->data + wpos, src, ret);

        if (ret < n) {
            /* start copying from beginning of dstfer */
            size_t remaining_bytes;

            capacity = rpos - 1;
            remaining_bytes = _min(capacity, n - ret);

            cc_memcpy(dst->data, (uint8_t *)src + ret, remaining_bytes);

            ret += remaining_bytes;
            wpos = remaining_bytes;
        } else {
            wpos += ret;
        }
    }

    _set_wpos(dst, wpos);

    return ret;
}

ssize_t
rbuf_read_fd(struct rbuf *src, int fd)
{
    uint32_t capacity;
    ssize_t ret;
    uint32_t rpos, wpos;
    rpos = _get_rpos(src);
    wpos = _get_wpos(src);

    if (wpos < rpos) {
        /* write until end, then wrap around */
        capacity = src->cap - rpos + 1;
        ret = write(fd, src->data + rpos, capacity);

        if (ret > 0) {
            rpos += ret;
        }

        if (ret == capacity) {
            /* more can be written, read from beginning of src */
            ssize_t remaining_bytes;

            capacity = wpos;
            remaining_bytes = write(fd, src->data, capacity);

            if (remaining_bytes >= 0) {
                rpos = remaining_bytes;
                ret += remaining_bytes;
            }
        }
    } else {
        /* no wrap around */
        capacity = wpos - rpos;
        ret = write(fd, src->data + rpos, capacity);

        if (ret > 0) {
            rpos += ret;
        }
    }

    _set_rpos(src, rpos);

    return ret;
}
