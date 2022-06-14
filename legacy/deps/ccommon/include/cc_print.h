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

#include <cc_util.h>

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

/*
 * cc_snprintf
 * cc_scnprintf
 * cc_vscnprintf
 */

#define cc_snprintf(_s, _n, ...)                                        \
    snprintf((char *)(_s), (size_t)(_n), __VA_ARGS__)

#define cc_scnprintf(_s, _n, ...)                                       \
    _scnprintf((char *)(_s), (size_t)(_n), __VA_ARGS__)

#define cc_vscnprintf(_s, _n, _f, _a)                                   \
    _vscnprintf((char *)(_s), (size_t)(_n), _f, _a)

/* behavior undefined if there isn't enough space in buf */
size_t cc_print_uint64_unsafe(char *buf, uint64_t n);
size_t cc_print_int64_unsafe(char *buf, int64_t n);

size_t cc_print_uint64(char *buf, size_t size, uint64_t n);
size_t cc_print_int64(char *buf, size_t size, int64_t n);

size_t _scnprintf(char *buf, size_t size, const char *fmt, ...);
size_t _vscnprintf(char *buf, size_t size, const char *fmt, va_list args);

static const uint64_t BASE10[CC_UINT64_MAXLEN - 1] = {
    0, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000,
    10000000000, 100000000000, 1000000000000, 10000000000000, 100000000000000,
    1000000000000000, 10000000000000000, 100000000000000000,
    1000000000000000000, 10000000000000000000ul};

static inline size_t
digits(uint64_t n) {
    size_t d = 1;

    while (n >= BASE10[d]) {
        d++;
    }

    return d;
}

#ifdef __cplusplus
}
#endif
