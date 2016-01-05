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

#include <buffer/cc_buf.h>

#include <cc_debug.h>
#include <cc_mm.h>
#include <cc_pool.h>


#define BUF_MODULE_NAME "ccommon::buffer:buf"

FREEPOOL(buf_pool, bufq, buf);
static struct buf_pool bufp;

static bool buf_init = false;
static bool bufp_init = false;

uint32_t buf_init_size = BUF_INIT_SIZE;
buf_metrics_st *buf_metrics = NULL;

void
buf_setup(uint32_t size, buf_metrics_st *metrics)
{
    log_info("setting up the %s module", BUF_MODULE_NAME);

    buf_init_size = size;
    buf_metrics = metrics;
    if (metrics != NULL) {
        BUF_METRIC_INIT(buf_metrics);
    }

    if (buf_init) {
        log_warn("%s was already setup, overwriting", BUF_MODULE_NAME);
    }

    buf_init = true;

    log_info("buf: size %zu", size);
}

void
buf_teardown(void)
{
    log_info("tear down the %s module", BUF_MODULE_NAME);

    if (!buf_init) {
        log_warn("%s was not setup but is being torn down", BUF_MODULE_NAME);
    }

    buf_metrics = NULL;
    buf_init = false;
}

void
buf_pool_create(uint32_t max)
{
    struct buf *buf;

    if (bufp_init) {
        log_warn("buf pool has already been created, ignoring");

        return;
    }

    log_info("creating buf pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&bufp, max);
    bufp_init = true;

    /**
     * NOTE: Right now I decide to preallocate if max != 0
     * whether we want an option where memory is capped but
     * not preallocated is a question for future exploration.
     * So far I see no point of that.
     */

    FREEPOOL_PREALLOC(buf, &bufp, max, next, buf_create);
    if (bufp.nfree < max) {
        log_crit("cannot preallocate buf pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

void
buf_pool_destroy(void)
{
    struct buf *buf, *nbuf;

    if (!bufp_init) {
        log_warn("buf pool was never created, ignoring destroy");

        return;
    }

    log_info("destroying buf pool: free %"PRIu32, bufp.nfree);

    FREEPOOL_DESTROY(buf, nbuf, &bufp, next, buf_destroy);
    bufp_init = false;
}

struct buf *
buf_borrow(void)
{
    struct buf *buf;

    FREEPOOL_BORROW(buf, &bufp, next, buf_create);

    if (buf == NULL) {
        log_warn("borrow buf failed, OOM or over limit");
        INCR(buf_metrics, buf_borrow_ex);

        return NULL;
    }

    buf_reset(buf);
    INCR(buf_metrics, buf_borrow);
    INCR(buf_metrics, buf_active);

    log_verb("borrow buf %p", buf);

    return buf;
}

void
buf_return(struct buf **buf)
{
    struct buf *elm;

    if (buf == NULL || (elm = *buf) == NULL || elm->free) {
        return;
    }

    ASSERT(STAILQ_NEXT(elm, next) == NULL);
    ASSERT(elm->wpos <= elm->end);

    log_verb("return buf %p", elm);

    elm->free = true;
    FREEPOOL_RETURN(elm, &bufp, next);

    *buf = NULL;
    INCR(buf_metrics, buf_return);
    DECR(buf_metrics, buf_active);
}

struct buf *
buf_create(void)
{
    struct buf *buf = (struct buf *)cc_alloc(buf_init_size);

    if (buf == NULL) {
        log_info("buf creation failed due to OOM");
        INCR(buf_metrics, buf_create_ex);

        return NULL;
    }

    buf->end = (char *)buf + buf_init_size;
    buf_reset(buf);
    INCR(buf_metrics, buf_create);
    INCR(buf_metrics, buf_curr);
    INCR_N(buf_metrics, buf_memory, buf_init_size);

    log_verb("created buf %p capacity %"PRIu32, buf, buf_capacity(buf));

    return buf;
}

void
buf_destroy(struct buf **buf)
{
    uint32_t cap;

    if (buf == NULL || *buf == NULL) {
        return;
    }

    cap = buf_capacity(*buf);
    log_verb("destroy buf %p capacity %"PRIu32, *buf, cap);

    cc_free(*buf);
    *buf = NULL;
    INCR(buf_metrics, buf_destroy);
    DECR(buf_metrics, buf_curr);
    DECR_N(buf_metrics, buf_memory, cap);
}
