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

#include <cc_define.h>
#include <cc_util.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* TODO(yao): separate byte string related functionalities into cc_bstring */
struct bstring {
    uint32_t len;   /* string length */
    char     *data; /* string data */
};

#define str2bstr(_str)  (struct bstring){ sizeof(_str) - 1, (_str) }
#define null_bstring    (struct bstring){ 0, NULL }

#define bstring_set_literal(_str, _literal) do {    \
    (_str)->len = (uint32_t)(sizeof(_literal) - 1); \
    (_str)->data = (_literal);                      \
} while (0);

#define bstring_set_cstr(_str, _cstr) do {       \
    (_str)->len = (uint32_t)(cc_strlen(_cstr));  \
    (_str)->data = (char *)(_cstr);              \
} while (0);

void bstring_init(struct bstring *str);
void bstring_deinit(struct bstring *str);
bool bstring_empty(const struct bstring *str);
rstatus_i bstring_duplicate(struct bstring *dst, const struct bstring *src);
rstatus_i bstring_copy(struct bstring *dst, const char *src, uint32_t srclen);
int bstring_compare(const struct bstring *s1, const struct bstring *s2);

struct bstring *bstring_alloc(uint32_t size);
void bstring_free(struct bstring **bstring);

/* efficient implementation of string comparion of short strings */
#define str2cmp(m, c0, c1)                                                     \
    (m[0] == c0 && m[1] == c1)

#define str3cmp(m, c0, c1, c2)                                                 \
    (m[0] == c0 && m[1] == c1 && m[2] == c2)

#define str4cmp(m, c0, c1, c2, c3)                                             \
    ((m[0] << 24 | m[1] << 16 | m[2] << 8 | m[3]) ==                           \
        ((c0 << 24) | (c1 << 16) | (c2 << 8) | c3))

#define str5cmp(m, c0, c1, c2, c3, c4)                                         \
    (str4cmp(m, c0, c1, c2, c3) && (m[4] == c4))

#define str6cmp(m, c0, c1, c2, c3, c4, c5)                                     \
    (str5cmp(m, c0, c1, c2, c3, c4) && m[5] == c5)

#define str7cmp(m, c0, c1, c2, c3, c4, c5, c6)                                 \
    (str6cmp(m, c0, c1, c2, c3, c4, c5) && m[6] == c6)

#define str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7)                             \
    (str4cmp(m, c0, c1, c2, c3) &&                                             \
        (m[4] << 24 | m[5] << 16 | m[6] << 8 | m[7]) ==                        \
        ((c4 << 24) | (c5 << 16) | (c6 << 8) | c7))

#define str9cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8)                         \
    (str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7) && m[8] == c8)

#define str10cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9)                    \
    (str9cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8) && m[9] == c9)

#define str11cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10)               \
    (str10cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9) && m[10] == c10)

#define str12cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11)          \
    (str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7) &&                             \
        (m[8] << 24 | m[9] << 16 | m[10] << 8 | m[11]) ==                      \
        ((c8 << 24) | (c9 << 16) | (c10 << 8) | c11))

/* below is a more efficient implementation for little-endian only, it takes
 * about 50% the cycles compared to the generic implementation above in the
 * extreme cases (e.g. string length being multiples of 4), however, our
 * profiling showed that string comparison does not contribute meaningfully to
 * overall processing cost, both events and hashes are far more notable, and
 * therefore we can choose the generic implementation until profiling results
 * indicate otherwise.
 */

/*
#define str4cmp(m, c0, c1, c2, c3)                                             \
    (*(uint32_t *) m == ((c3 << 24) | (c2 << 16) | (c1 << 8) | c0))

#define str5cmp(m, c0, c1, c2, c3, c4)                                         \
    (str4cmp(m, c0, c1, c2, c3) && (m[4] == c4))

#define str6cmp(m, c0, c1, c2, c3, c4, c5)                                     \
    (str4cmp(m, c0, c1, c2, c3) &&                                             \
        (((uint32_t *) m)[1] & 0xffff) == ((c5 << 8) | c4))

#define str7cmp(m, c0, c1, c2, c3, c4, c5, c6)                                 \
    (str6cmp(m, c0, c1, c2, c3, c4, c5) && (m[6] == c6))

#define str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7)                             \
    (str4cmp(m, c0, c1, c2, c3) &&                                             \
        (((uint32_t *) m)[1] == ((c7 << 24) | (c6 << 16) | (c5 << 8) | c4)))

#define str9cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8)                         \
    (str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7) && m[8] == c8)

#define str10cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9)                    \
    (str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7) &&                             \
        (((uint32_t *) m)[2] & 0xffff) == ((c9 << 8) | c8))

#define str11cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10)               \
    (str10cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9) && (m[10] == c10))

#define str12cmp(m, c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11)          \
    (str8cmp(m, c0, c1, c2, c3, c4, c5, c6, c7) &&                             \
        (((uint32_t *) m)[2] == ((c11 << 24) | (c10 << 16) | (c9 << 8) | c8)))
*/

/*
 * Wrapper around common routines for manipulating C character strings
 *
 * cc_memcmp
 * cc_memcpy
 * cc_memmove
 * cc_memchr
 * cc_memset
 * cc_bcmp
 */
#define cc_memcmp(_p1, _p2, _n)                                 \
    memcmp(_p1, _p2, (size_t)(_n))

#define cc_memcpy(_d, _c, _n)                                   \
    memcpy(_d, _c, (size_t)(_n))

#define cc_memmove(_d, _c, _n)                                  \
    memmove(_d, _c, (size_t)(_n))

#define cc_memchr(_d, _c, _n)                                   \
    memchr(_d, _c, (size_t)(_n))

#define cc_memset(_p, _v, _n)                                   \
    memset(_p, _v, (size_t)(_n))

#define cc_bcmp(_s1, _s2, _n)                                   \
    bcmp((char *)(_s1), (char *)(_s2), (size_t)(_n))


/* bstring to uint conversion */
rstatus_i bstring_atou64(uint64_t *u64, struct bstring *str);

#ifdef __cplusplus
}
#endif
