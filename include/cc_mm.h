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

#include <cc_define.h>

#include <stddef.h>

/*
 * Memory allocation and free wrappers with debugging information.
 *
 * cc_alloc
 * cc_zalloc
 * cc_calloc
 * cc_realloc
 *
 * cc_free
 *
 * cc_mmap
 * cc_munmap
 */
#define cc_alloc(_s)                                            \
    _cc_alloc((size_t)(_s), __FILE__, __LINE__)

#define cc_zalloc(_s)                                           \
    _cc_zalloc((size_t)(_s), __FILE__, __LINE__)

#define cc_calloc(_n, _s)                                       \
    _cc_calloc((size_t)(_n), (size_t)(_s), __FILE__, __LINE__)

#if defined CC_DEBUG_MM && CC_DEBUG_MM == 1
#define cc_realloc(_p, _s)                                      \
    _cc_realloc_move(_p, (size_t)(_s), __FILE__, __LINE__)
#else
#define cc_realloc(_p, _s)                                      \
    _cc_realloc(_p, (size_t)(_s), __FILE__, __LINE__)
#endif

#define cc_free(_p) do {                                        \
    _cc_free(_p, __FILE__, __LINE__);                           \
    (_p) = NULL;                                                \
} while (0)

#define cc_mmap(_s)                                             \
    _cc_mmap((size_t)(_s), __FILE__, __LINE__)

#define cc_munmap(_p, _s)                                       \
    _cc_munmap(_p, (size_t)(_s), __FILE__, __LINE__)

void * _cc_alloc(size_t size, const char *name, int line);
void * _cc_zalloc(size_t size, const char *name, int line);
void * _cc_calloc(size_t nmemb, size_t size, const char *name, int line);
void * _cc_realloc(void *ptr, size_t size, const char *name, int line);
void * _cc_realloc_move(void *ptr, size_t size, const char *name, int line);
void _cc_free(void *ptr, const char *name, int line);
void * _cc_mmap(size_t size, const char *name, int line);
int _cc_munmap(void *p, size_t size, const char *name, int line);
