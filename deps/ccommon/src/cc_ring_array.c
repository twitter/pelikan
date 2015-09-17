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

#include <cc_ring_array.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

#include <stdbool.h>

#define RING_ARRAY_MODULE_NAME "ccommon::ring_array"

static bool ring_array_init = false;

#define RING_ARRAY_HDR_SIZE   offsetof(struct ring_array, data)

/**
 * The total number of slots allocated is (cap + 1)
 *
 * Each ring array should have exactly one reader and exactly one writer, as
 * far as threads are concerned (which can be the same). This allows the use of
 * atomic instructions to replace locks.
 *
 * We use an extra slot to differentiate full from empty.
 *
 * 1) If rpos == wpos, the buffer is empty.
 *
 * 2) If rpos is behind wpos (see below):
 *     # of occupied slots: wpos - rpos
 *     # of vacant slots: rpos + cap - wpos + 1
 *     # of writable slots: rpos + cap - wpos
 *     full if: rpos == 0, wpos == cap
 *
 *       0                       cap
 *       |                       |
 *       v                       v
 *      +-+-+-+---------------+-+-+
 * data | | | |      ...      | | |
 *      +-+-+-+---------------+-+-+
 *           ^             ^
 *           |             |
 *           rpos          wpos
 *
 * 3) If rpos is ahead of wpos (see below):
 *     # of occupied slots: wpos + cap - rpos + 1
 *     # of vacant slots: rpos - wpos
 *     # of writable slots: rpos - wpos - 1
 *     full if: rpos == wpos + 1
 *
 *       0                       cap
 *       |                       |
 *       v                       v
 *      +-+-+-+---------------+-+-+
 * data | | | |      ...      | | |
 *      +-+-+-+---------------+-+-+
 *           ^             ^
 *           |             |
 *           wpos          rpos
 *
 */


static inline uint32_t
ring_array_nelem(uint32_t rpos, uint32_t wpos, uint32_t cap)
{
    if (rpos <= wpos) { /* condition 1), 2) */
        return wpos - rpos;
    } else {            /* condition 3) */
        return wpos + (cap - rpos + 1);
    }
}

static inline bool
ring_array_empty(uint32_t rpos, uint32_t wpos)
{
    return rpos == wpos;
}

static inline bool
ring_array_full(uint32_t rpos, uint32_t wpos, uint32_t cap)
{
    return ring_array_nelem(rpos, wpos, cap) == cap;
}

rstatus_t
ring_array_push(const void *elem, struct ring_array *arr)
{
    /**
     * Take snapshot of rpos, since another thread might be popping. Note: other
     * members of arr do not need to be saved because we assume the other thread
     * only pops and does not push; in other words, only one thread updates
     * either rpos or wpos.
     */
    uint32_t new_wpos;
    uint32_t rpos = __atomic_load_n(&(arr->rpos), __ATOMIC_RELAXED);

    if (ring_array_full(rpos, arr->wpos, arr->cap)) {
        log_debug("Could not push to ring array %p; array is full", arr);
        return CC_ERROR;
    }

    cc_memcpy(arr->data + (arr->elem_size * arr->wpos), elem, arr->elem_size);

    /* update wpos atomically */
    new_wpos = (arr->wpos + 1) % arr->cap;
    __atomic_store_n(&(arr->wpos), new_wpos, __ATOMIC_RELAXED);

    return CC_OK;
}

rstatus_t
ring_array_pop(void *elem, struct ring_array *arr)
{
    /* take snapshot of wpos, since another thread might be pushing */
    uint32_t new_rpos;
    uint32_t wpos = __atomic_load_n(&(arr->wpos), __ATOMIC_RELAXED);

    if (ring_array_empty(arr->rpos, wpos)) {
        log_debug("Could not pop from ring array %p; array is empty", arr);
        return CC_ERROR;
    }

    if (elem != NULL) {
        cc_memcpy(elem, arr->data + (arr->elem_size * arr->rpos), arr->elem_size);
    }

    /* update rpos atomically */
    new_rpos = (arr->rpos + 1) % arr->cap;
    __atomic_store_n(&(arr->rpos), new_rpos, __ATOMIC_RELAXED);

    return CC_OK;
}

struct ring_array *
ring_array_create(size_t elem_size, uint32_t cap)
{
    struct ring_array *arr;

    arr = cc_alloc(RING_ARRAY_HDR_SIZE + elem_size * (cap + 1));

    if (arr == NULL) {
        log_error("Could not allocate memory for ring array cap %u "
                  "elem_size %u", cap, elem_size);
        return NULL;
    }

    arr->elem_size = elem_size;
    arr->cap = cap;
    arr->rpos = arr->wpos = 0;
    return arr;
}

void
ring_array_destroy(struct ring_array *arr)
{
    log_verb("destroying ring array %p and freeing memory");
    cc_free(arr);
}

void
ring_array_setup(void)
{
    log_info("set up the %s module", RING_ARRAY_MODULE_NAME);
    if (ring_array_init) {
        log_warn("%s has already been setup, overwrite", RING_ARRAY_MODULE_NAME);
    }
    ring_array_init = true;
}

void
ring_array_teardown(void)
{
    log_info("tear down the %s module", RING_ARRAY_MODULE_NAME);
    if (!ring_array_init) {
        log_warn("%s has never been setup", RING_ARRAY_MODULE_NAME);
    }
    ring_array_init = false;
}
