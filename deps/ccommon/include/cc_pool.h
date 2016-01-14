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

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_debug.h>
#include <cc_queue.h>

#include <inttypes.h>
#include <stdbool.h>

#define FREEPOOL(pool, name, type)                                  \
STAILQ_HEAD(name, type);                                            \
struct pool {                                                       \
    struct name     freeq;                                          \
    uint32_t        nfree;                                          \
    uint32_t        nused;                                          \
    uint32_t        nmax;                                           \
    bool            initialized;                                    \
}

#define FREEPOOL_CREATE(pool, max) do {                             \
    ASSERT(!(pool)->initialized);                                   \
    STAILQ_INIT(&(pool)->freeq);                                    \
    (pool)->nmax = (max) > 0 ? (max) : UINT32_MAX;                  \
    (pool)->nfree = 0;                                              \
    (pool)->nused = 0;                                              \
    (pool)->initialized = true;                                     \
} while (0)

#define FREEPOOL_DESTROY(var, tvar, pool, field, destroy) do {      \
    ASSERT((pool)->initialized);                                    \
    ASSERT((pool)->nused == 0);                                     \
    STAILQ_FOREACH_SAFE(var, &(pool)->freeq, field, tvar) {         \
        STAILQ_REMOVE_HEAD(&(pool)->freeq, next);                   \
        (pool)->nfree--;                                            \
        destroy(&var);                                              \
    }                                                               \
    (pool)->initialized = false;                                    \
    ASSERT((pool)->nfree == 0);                                     \
    ASSERT(STAILQ_EMPTY(&(pool)->freeq));                           \
} while (0)

#define FREEPOOL_PREALLOC(var, pool, size, field, create) do {      \
    ASSERT((pool)->initialized);                                    \
    while ((pool)->nfree < size) {                                  \
        (var) = create();                                           \
        if ((var) != NULL) {                                        \
            STAILQ_INSERT_HEAD(&(pool)->freeq, var, field);         \
            (pool)->nfree++;                                        \
        } else {                                                    \
            break;                                                  \
        }                                                           \
    }                                                               \
} while (0)

#define FREEPOOL_BORROW(var, pool, field, create) do {              \
    ASSERT((pool)->initialized);                                    \
    if (!STAILQ_EMPTY(&(pool)->freeq)) {                            \
        (var) = STAILQ_FIRST(&(pool)->freeq);                       \
        STAILQ_REMOVE_HEAD(&(pool)->freeq, field);                  \
        (pool)->nfree--;                                            \
    } else if ((pool)->nfree + (pool)->nused < (pool)->nmax) {      \
        (var) = create();                                           \
    } else {                                                        \
        (var) = NULL;                                               \
    }                                                               \
    if ((var) != NULL) {                                            \
        (pool)->nused++;                                            \
        STAILQ_NEXT((var), field) = NULL;                           \
    }                                                               \
} while (0)

#define FREEPOOL_RETURN(var, pool, field) do {                      \
    ASSERT((pool)->initialized);                                    \
    STAILQ_INSERT_HEAD(&(pool)->freeq, var, field);                 \
    (pool)->nfree++;                                                \
    (pool)->nused--;                                                \
} while (0)

#ifdef __cplusplus
}
#endif
