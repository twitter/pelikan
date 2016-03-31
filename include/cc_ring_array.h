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

/*
 * This ring array is designed specifically with communication between two
 * threads in mind, with one thread as the producer and the other thread as the
 * consumer. In other words, one thread does all of the pushing and the other
 * thread does all of the popping. Given these conditions are met, the ring
 * array can guarantee that all pushes and pops will be valid and leave the
 * array in a valid state.
 */

#pragma once

#include <cc_define.h>

#include <stddef.h>
#include <stdint.h>

#define RING_ARRAY_DEFAULT_CAP 1024

struct ring_array {
    size_t      elem_size;         /* element size */
    uint32_t    cap;               /* total capacity (# items stored + 1) */
    uint32_t    rpos;              /* read offset */
    uint32_t    wpos;              /* write offset */
    union {
        size_t  pad;               /* using a size_t member to force alignment at
                                      native word boundary */
        uint8_t data[1];           /* beginning of array */
    };
};

/* push an element into the array */
rstatus_i ring_array_push(const void *elem, struct ring_array *arr);

/* pop an element from the array */
rstatus_i ring_array_pop(void *elem, struct ring_array *arr);

/* creation/destruction */
struct ring_array *ring_array_create(size_t elem_size, uint32_t cap);
void ring_array_destroy(struct ring_array *arr);
