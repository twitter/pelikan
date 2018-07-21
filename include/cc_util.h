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

#include <stdint.h>
#include <sys/param.h>
#include <sys/types.h>
#include <sys/uio.h>
#include <unistd.h>

/*
 * This file should be further split up as the individual sections grow.
 * Candidates:
 *   - string/char
 *   - int/unit
 *   - math
 *   - network
 */

/* syntax */
#define LF              (uint8_t) 10
#define CR              (uint8_t) 13
#define CRLF            "\r\n"
#define CRLF_LEN        (uint32_t) (sizeof(CRLF) - 1)

/* unit */
#define KiB             1024
#define MiB             (1024 * KiB)
#define GiB             (1024 * MiB)

/* int */
/*
 * Length of 1 byte, 2 bytes, 4 bytes, 8 bytes and largest integral
 * type (uintmax_t) in ascii, including the null terminator '\0'
 *
 * From stdint.h, we have:
 * # define UINT8_MAX   (255)
 * # define UINT16_MAX  (65535)
 * # define UINT32_MAX  (4294967295U)
 * # define UINT64_MAX  (__UINT64_C(18446744073709551615))
 *
 * # define INT64_MIN   -9223372036854775808LL
 */
#define CC_UINT8_MAXLEN     (3 + 1)
#define CC_UINT16_MAXLEN    (5 + 1)
#define CC_UINT32_MAXLEN    (10 + 1)
#define CC_UINT64_MAXLEN    (20 + 1)
#define CC_UINTMAX_MAXLEN   CC_UINT64_MAXLEN

#define CC_INT64_MAXLEN    (1 + 19 + 1)

/* alignment */
/* Make data 'd' or pointer 'p', n-byte aligned, where n is a power of 2 */
#define CC_ALIGNMENT        sizeof(unsigned long) /* platform word */
#define CC_ALIGN(d, n)      ((size_t)(((d) + (n - 1)) & ~(n - 1)))
#define CC_ALIGN_PTR(p, n)  \
    (void *) (((uintptr_t) (p) + ((uintptr_t) n - 1)) & ~((uintptr_t) n - 1))

/* string */
/*
 * This stringifies both a regular variable/value and a macro-defined symbol,
 * the latter needs another level of macro to pre-expand properly
 */
#define str(_s) _str(_s)
#define _str(_s) #_s

#define cc_strlen(_s)                                           \
    strlen((char *)(_s))

#define cc_strcmp(_s1, _s2)                                     \
    strcmp((char *)(_s1), (char *)(_s2))

#define cc_strncmp(_s1, _s2, _n)                                \
    strncmp((char *)(_s1), (char *)(_s2), (size_t)(_n))

#define cc_strndup(_s, _n)                                      \
    (uint8_t *)strndup((char *)(_s), (size_t)(_n));

#define cc_strchr(_p, _l, _c)                                   \
    _cc_strchr((uint8_t *)(_p), (uint8_t *)(_l), (uint8_t)(_c))

#define cc_strrchr(_p, _s, _c)                                  \
    _cc_strrchr((uint8_t *)(_p),(uint8_t *)(_s), (uint8_t)(_c))

static inline uint8_t *
_cc_strchr(uint8_t *p, uint8_t *last, uint8_t c)
{
    while (p < last) {
        if (*p == c) {
            return p;
        }
        p++;
    }

    return NULL;
}

static inline uint8_t *
_cc_strrchr(uint8_t *p, uint8_t *start, uint8_t c)
{
    while (p >= start) {
        if (*p == c) {
            return p;
        }
        p--;
    }

    return NULL;
}

/* math */
#define SQUARE(d)           ((d) * (d))
#define VAR(s, s2, n)       (((n) < 2) ? 0.0 : ((s2) - SQUARE(s)/(n)) / ((n) - 1))
#define STDDEV(s, s2, n)    (((n) < 2) ? 0.0 : sqrt(VAR((s), (s2), (n))))

/* The following functions are only valid for x >= 0, step >= 1. */
/* Return 'x' rounded up to the nearest multiple of 'step'. */
#define ROUND_UP(x, step)   (((x) + (step) - 1) / (step) * (step))
/* Return 'x' rounded down to the nearest multiple of 'step'. */
#define ROUND_DOWN(x, step) ((x) / (step) * (step))

/* network */
#define CC_INET4_ADDRSTRLEN (sizeof("255.255.255.255") - 1)
#define CC_INET6_ADDRSTRLEN \
    (sizeof("ffff:ffff:ffff:ffff:ffff:ffff:255.255.255.255") - 1)
#define CC_INET_ADDRSTRLEN  MAX(CC_INET4_ADDRSTRLEN, CC_INET6_ADDRSTRLEN)
#define CC_UNIX_ADDRSTRLEN  \
    (sizeof(struct sockaddr_un) - offsetof(struct sockaddr_un, sun_path))
#define CC_MAXHOSTNAMELEN   256

#ifdef __cplusplus
}
#endif
