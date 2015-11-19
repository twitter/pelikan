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

/*
 * buf: a buffer base for contiguous buffers that can be pooled together
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_define.h>
#include <cc_metric.h>
#include <cc_queue.h>
#include <cc_util.h>

#include <stdbool.h>
#include <sys/param.h>

#define BUF_OPTION(ACTION)                                                                                \
    ACTION( buf_init_size,  OPTION_TYPE_UINT,  str(BUF_DEFAULT_SIZE), "default size when buf is created" )\
    ACTION( buf_poolsize,   OPTION_TYPE_UINT,  str(BUF_POOLSIZE),     "buf pool size"                    )

/*          name              type            description */
#define BUF_METRIC(ACTION)                                              \
    ACTION( buf_curr,         METRIC_GUAGE,   "# buf allocated"        )\
    ACTION( buf_active,       METRIC_GUAGE,   "# buf in use/borrowed"  )\
    ACTION( buf_create,       METRIC_COUNTER, "# buf creates"          )\
    ACTION( buf_create_ex,    METRIC_COUNTER, "# buf create exceptions")\
    ACTION( buf_destroy,      METRIC_COUNTER, "# buf destroys"         )\
    ACTION( buf_borrow,       METRIC_COUNTER, "# buf borrows"          )\
    ACTION( buf_borrow_ex,    METRIC_COUNTER, "# buf borrow exceptions")\
    ACTION( buf_return,       METRIC_COUNTER, "# buf returns"          )\
    ACTION( buf_memory,       METRIC_GAUGE,   "memory allocated to buf")

typedef struct {
    BUF_METRIC(METRIC_DECLARE)
} buf_metrics_st;

#define BUF_METRIC_INIT(_metrics) do {                          \
    *(_metrics) = (buf_metrics_st) { BUF_METRIC(METRIC_INIT) }; \
} while(0)

struct buf {
    STAILQ_ENTRY(buf) next;     /* next buf in pool */
    char              *rpos;    /* read marker */
    char              *wpos;    /* write marker */
    char              *end;     /* end of buffer */
    bool              free;     /* is this buf free? */
    char              begin[1]; /* beginning of buffer */
};

#define BUF_HDR_SIZE       offsetof(struct buf, begin)
#define BUF_DEFAULT_SIZE   16 * KiB
#define BUF_POOLSIZE       0    /* unlimited */

STAILQ_HEAD(buf_sqh, buf); /* corresponding header type for the STAILQ */

extern uint32_t buf_init_size;
extern buf_metrics_st *buf_metrics;

#define BUF_INIT_SIZE (16 * KiB)
#define BUF_POOLSIZE     0 /* unlimited */

#define BUF_EMTPY(BUF) \
    ((BUF)->rpos == (BUF)->wpos)

#define BUF_FULL(BUF) \
    ((BUF)->wpos == (BUF)->end)

/* Setup/teardown buf module */
void buf_setup(uint32_t size, buf_metrics_st *metrics);
void buf_teardown(void);

/* Create/destroy buffer pool */
void buf_pool_create(uint32_t max);
void buf_pool_destroy(void);

/* Obtain/return a buffer from the pool */
struct buf *buf_borrow(void);
void buf_return(struct buf **buf);

/* Create/destroy a buffer (allocate/deallocate) */
struct buf *buf_create(void);
void buf_destroy(struct buf **buf);

/* Size of data that has yet to be read */
static inline uint32_t
buf_rsize(struct buf *buf)
{
    ASSERT(buf->rpos <= buf->wpos);

    return (uint32_t)(buf->wpos - buf->rpos);
}

/* Amount of room left in buffer for writing new data */
static inline uint32_t
buf_wsize(struct buf *buf)
{
    ASSERT(buf->wpos <= buf->end);

    return (uint32_t)(buf->end - buf->wpos);
}

/* Total capacity of given buf */
static inline uint32_t
buf_size(struct buf *buf)
{
    ASSERT(buf->begin < buf->end);

    return (uint32_t)(buf->end - (char*)buf);
}

static inline uint32_t
buf_capacity(struct buf *buf)
{
    ASSERT(buf->begin < buf->end);

    return (uint32_t)(buf->end - buf->begin);
}

/* new capacity needed to append count bytes to the buffer */
static inline uint32_t
buf_new_cap(struct buf *buf, uint32_t count)
{
    ASSERT(buf->begin <= buf->wpos);

    return buf->wpos - buf->begin + count;
}

static inline void
buf_reset(struct buf *buf)
{
    STAILQ_NEXT(buf, next) = NULL;
    buf->free = 0;
    buf->rpos = buf->wpos = buf->begin;
}

static inline uint32_t
buf_read(char *dst, struct buf *src, uint32_t count)
{
    ASSERT(dst != NULL && src != NULL);

    uint32_t len = MIN(buf_rsize(src), count);

    cc_memcpy(dst, src->rpos, len);
    src->rpos += len;

    return len;
}

static inline uint32_t
buf_write(struct buf *dst, char *src, uint32_t count)
{
    ASSERT(dst != NULL && src != NULL);

    uint32_t len = MIN(buf_wsize(dst), count);

    cc_memcpy(dst->wpos, src, len);
    dst->wpos += len;

    return len;
}

static inline void
buf_lshift(struct buf *buf)
{
    ASSERT(buf != NULL);

    uint32_t size = buf_rsize(buf);

    if (size > 0) {
        cc_memmove(buf->begin, buf->rpos, size);
    }

    buf->rpos = buf->begin;
    buf->wpos = buf->begin + size;
}

static inline void
buf_rshift(struct buf *buf)
{
    ASSERT(buf != NULL);

    uint32_t size = buf_rsize(buf);

    if (size > 0) {
        cc_memmove(buf->end - size, buf->rpos, size);
    }

    buf->rpos = buf->end - size;
    buf->wpos = buf->end;
}

#ifdef __cplusplus
}
#endif
