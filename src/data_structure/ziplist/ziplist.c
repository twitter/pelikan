#include "ziplist.h"

#include <cc_debug.h>

/* The description of the ziplist is adapted from the redis project with heavy
 * modification. License is included for comments only.
 * The binary does not contain any source code from Redis.
 */


/* The ziplist is a specially encoded dually linked list that is designed
 * to be very memory efficient. It stores both strings and integer values. In
 * the current implementation, insertion and removal gets slower as the entry
 * gets closer to the center of the list.
 *
 * ----------------------------------------------------------------------------
 *
 * ZIPLIST OVERALL LAYOUT
 * ======================
 *
 * The general layout of the ziplist is as follows:
 *
 * <nentry> <zlend> <entry> <entry> ... <entry>
 * ╰--------------╯ ╰-------------------------╯
 *       header                 body
 *
 * Overhead: 8 bytes
 *
 * <uint32_t nentry> is the number of entries.
 *
 * <uint32_t zlend> is the offset to end of the last entry in the list. This
 * allows a pop operation on the far side of the list without the need for full
 * traversal. Note 0 starts from the beginning of the header, and the smallest
 * entry is 2 bytes, so zlend less than 9 indicate an empty list.
 *
 *
 * ZIPLIST ENTRIES
 * ===============
 *
 * Every entry in the ziplist contains the encoding, which indicates type and
 * length, value of the entry, followed by length of the entry as at the end.
 * When scanning forward, the encoding provides the length of the entry, while
 * the same information can be obtained by reading the last byte of the entry
 * if traversing backward.
 *
 * A complete entry is stored like this:
 *
 * <encoding> <data> <len>
 *
 * Sometimes the encoding represents the entry itself, like for small integers
 * as we'll see later. In such a case the <entry-data> part is missing, and we
 * could have just:
 *
 * <encoding/data> <len>
 *
 * len takes exactly 1 byte, as we only cater to smaller entries for now.
 *
 * The encoding and value of the entry are content-dependent.
 * <= 250 : 1 byte, no memory overhead
 *      unsigned integer up to 250
 * == 251 : 3 (1+2) bytes, 50.0% overhead
 *      unsigned integer up to (2^16 - 1)
 * == 252 : 4 (1+3) bytes, 33.3% overhead
 *      unsigned integer up to (2^24 - 1)
 * == 253 : 8 (1+7) bytes, 14.3% overhead
 *      unsigned integer up to (2^56 - 1)
 * == 254 : 9 (1+8) bytes, 12.5% overhead
 *      unsigned integer up to (2^64 - 1)
 * == 255 : (1 + 1 + N) bytes, upto 200% overhead for 1-byte strings, but that
*           can be stored as integer to avoid this overhead
 *      string up to 252 bytes (yields a 255 byte zipentry)
 *
 * This encoding is different from ziplist in Redis, which optimizes for small
 * strings (1 byte overhead instead of 2) instead of small integers. We do it
 * differently because in practice it seems values small in size tend to be
 * numerical in nature, so we decide to optimize for storing small integers
 * efficiently instead.
 * We also don't attempt to accommodate large values as ziplist entries, because
 * the operations on large values generally have very different considerations
 * from small ones. For example, it is much more important to make sure memory
 * operations are efficient (such as resizing and copying) when updating large
 * values, and the overhead of encoding becomes encoding. They also will have
 * very different runtime characteristics. So instead of supporting all value
 * sizes in theory and running into operational issues later, it is better,
 * at least operationally, to make such limitations explicit and deal with
 * different use cases separately.
 *
 * TODO: optimization if all list members are of the same size, then we can
 * remove the entry header all together and seeking will be extremely easy.
 *
 *
 * EXAMPLE
 * =======
 *
 * The following is a ziplist containing the two elements representing
 * the integer 2 and string "pi". It is composed of 15 bytes, that we visually
 * split into sections:
 *
 *  [02 00 00 00] [0e 00 00 00] [03 02] [ff 02 70 69 05]
 *  ╰-----------╯ ╰-----------╯ ╰-----╯ ╰--------------╯
 *        2             14         3          "pi"
 *
 * The first 4 bytes represent the number 2, that is the number of entries
 * the whole ziplist is composed of. The second 4 bytes are the offset
 * at which the end of ziplist entries is found.
 *
 * Next is the body, "03 02" as the first entry representing the number 2. It
 * starts with the byte 0x03 which corresponds to the encoding of the small
 * integer, the 0x02 is the length of the current entry. The next entry, "pi",
 * has an encoding byte of value 0xff (255), and a length of 5 bytes, the
 * content "pi" is stored between these two values, whose hex form is 0x70 0x69.
 *
 * ----------------------------------------------------------------------------
 *
 * Copyright (c) 2009-2012, Pieter Noordhuis <pcnoordhuis at gmail dot com>
 * Copyright (c) 2009-2017, Salvatore Sanfilippo <antirez at gmail dot com>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 *   * Redistributions of source code must retain the above copyright notice,
 *     this list of conditions and the following disclaimer.
 *   * Redistributions in binary form must reproduce the above copyright
 *     notice, this list of conditions and the following disclaimer in the
 *     documentation and/or other materials provided with the distribution.
 *   * Neither the name of Redis nor the names of its contributors may be used
 *     to endorse or promote products derived from this software without
 *     specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE
 * LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

static uint8_t ze_buf[UINT8_MAX];


static inline uint8_t *
_ziplist_end(ziplist_p zl)
{
    return zl + ZL_NEND(zl);
}

/* zipentry size required for a value */
static inline uint8_t
_encode_size(const struct blob *val)
{
    if (val->type == BLOB_TYPE_STR) {
        return ZE_ZELEN_LEN + ZE_STR_HEADER + val->vstr.len;
    } else { /* BLOB_TYPE_INT */
        if (val->vint <= ZE_U8_MAX) {
            return ZE_ZELEN_LEN + ZE_U8_LEN;
        } else if (val->vint <= ZE_U16_MAX) {
            return ZE_ZELEN_LEN + ZE_U16_LEN;
        } else if (val->vint <= ZE_U24_MAX) {
            return ZE_ZELEN_LEN + ZE_U24_LEN;
        } else if (val->vint <= ZE_U56_MAX) {
            return ZE_ZELEN_LEN + ZE_U56_LEN;
        } else { /* ZE_U64_MAX */
            return ZE_ZELEN_LEN + ZE_U64_LEN;
        }
    }
}

static inline uint8_t
_zipentry_len(zipentry_p ze)
{
    if (*ze <= ZE_U8_MAX) {
        return ZE_ZELEN_LEN + ZE_U8_LEN;
    } else if (*ze == ZE_U16) {
        return ZE_ZELEN_LEN + ZE_U16_LEN;
    } else if (*ze == ZE_U24) {
        return ZE_ZELEN_LEN + ZE_U24_LEN;
    } else if (*ze == ZE_U56) {
        return ZE_ZELEN_LEN + ZE_U56_LEN;
    } else if (*ze == ZE_U64) {
        return ZE_ZELEN_LEN + ZE_U64_LEN;
    } else { /* ZE_STR */
        return ZE_ZELEN_LEN + ZE_STR_HEADER + *(ze + 1);
    }
}

static inline uint64_t
_zipentry_int(zipentry_p ze) {
    if (*ze <= ZE_U8_MAX) {
        return *((uint8_t*)ze);
    } else if (*ze == ZE_U16) {
        return *((uint16_t*)(ze + 1));
    } else if (*ze == ZE_U24) {
        return *((uint32_t*)ze) >> 8;
    } else if (*ze == ZE_U56) {
        return *((uint64_t*)ze) >> 8;
    } else if (*ze == ZE_U64) {
        return *((uint64_t*)(ze + 1));
    } else {
        NOT_REACHED();
        return ZE_U64_MAX;
    }
}

static inline struct bstring
_zipentry_str(zipentry_p ze) {
    ASSERT(*ze == ZE_STR);

    /* .len, .data */
    return (struct bstring){*(ze + 1), (char *)ze + 2};
}

ziplist_rstatus_e
zipentry_size(uint8_t *sz, struct blob *val)
{
    if (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN) {
        return ZIPLIST_EINVALID;
    }

    *sz = _encode_size(val);

    return ZIPLIST_OK;
}

int
zipentry_compare(const zipentry_p ze, const struct blob *val)
{
    ASSERT(ze != NULL && val != NULL);

    struct blob zev;

    zipentry_get(&zev, ze);

    return blob_compare(&zev, val);
}

ziplist_rstatus_e
zipentry_get(struct blob *val, const zipentry_p ze)
{
    if (ze == NULL) {
        return ZIPLIST_ERROR;
    }

    if (*ze == ZE_STR) {
        val->type = BLOB_TYPE_STR;
        val->vstr = _zipentry_str(ze);
    } else {
        val->type = BLOB_TYPE_INT;
        val->vint = _zipentry_int(ze);
    }

    return ZIPLIST_OK;
}

static inline uint32_t
_zipentry_encode(zipentry_p ze, const struct blob *val)
{
    uint8_t len = _encode_size(val);

    if (val->type == BLOB_TYPE_STR) {
        *ze = ZE_STR;
        *(ze + 1) = val->vstr.len;
        cc_memcpy(ze + 2, val->vstr.data, val->vstr.len);
    } else {
        if (val->vint <= ZE_U8_MAX) {
            *ze = val->vint;
        } else if (val->vint <= ZE_U16_MAX) {
            *ze = ZE_U16;
            *((uint16_t*)(ze + 1)) = (uint16_t)val->vint;
        } else if (val->vint <= ZE_U24_MAX) {
            *((uint32_t*)ze) = (uint32_t)((val->vint << 8) + ZE_U24);
        } else if (val->vint <= ZE_U56_MAX) {
            *((uint64_t*)ze) = (uint64_t)((val->vint << 8) + ZE_U56);
        } else { /* 64-bit uint */
            *ze = ZE_U64;
            *((uint64_t*)(ze + 1)) = val->vint;
        }
    }

    *(ze + len - 1) = len; /* set len at the end */

    return len;
}

ziplist_rstatus_e
zipentry_set(zipentry_p ze, const struct blob *val)
{
    if (ze == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN) {
        return ZIPLIST_EINVALID;
    }

    _zipentry_encode(ze, val);

    return ZIPLIST_OK;
}


static inline zipentry_p
_ziplist_head(ziplist_p zl)
{
    return zl + ZIPLIST_HEADER_SIZE;
}

static inline zipentry_p
_ziplist_tail(ziplist_p zl)
{
    uint8_t *p = _ziplist_end(zl);

    return p - *p + 1;
}

ziplist_rstatus_e
ziplist_reset(ziplist_p zl)
{
    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    ZL_NENTRY(zl) = 0;
    ZL_NEND(zl) = 7;

    return ZIPLIST_OK;
}

/* do NOT call this function on the first zip entry, use ziplist_prev */
static inline zipentry_p
_ziplist_prev(zipentry_p ze)
{
    return ze - *(ze - 1); /* *(ze - 1) : length of the previous entry */
}

/* do NOT call this function on the last zip entry, use ziplist_prev */
static inline zipentry_p
_ziplist_next(zipentry_p ze)
{
    return ze + _zipentry_len(ze);
}

ziplist_rstatus_e
ziplist_prev(zipentry_p *ze, ziplist_p zl, const zipentry_p curr)
{
    if (curr == _ziplist_head(zl)) {
        return ZIPLIST_EOOB;
    } else {
        *ze = _ziplist_prev(curr);
        return ZIPLIST_OK;
    }
}

ziplist_rstatus_e
ziplist_next(zipentry_p *ze, ziplist_p zl, const zipentry_p curr)
{
    if (curr == _ziplist_tail(zl)) {
        return ZIPLIST_EOOB;
    } else {
        *ze = _ziplist_next(curr);
        return ZIPLIST_OK;
    }
}

static inline zipentry_p
_ziplist_fromleft(const ziplist_p zl, uint32_t idx)
{
    zipentry_p ze = _ziplist_head(zl);

    for (; idx > 0; idx--, ze += _zipentry_len(ze));

    return ze;
}

static inline zipentry_p
_ziplist_fromright(const ziplist_p zl, uint32_t idx)
{
    uint8_t *p = _ziplist_end(zl);

    for (; idx > 0; idx--, p -= *p);

    return p - *p + 1;
}

ziplist_rstatus_e
ziplist_locate(zipentry_p *ze, const ziplist_p zl, int64_t idx)
{
    uint32_t nentry;

    if (zl == NULL || ze == NULL) {
        return ZIPLIST_ERROR;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (nentry <= idx) {
        *ze = NULL;
        return ZIPLIST_EOOB;
    }

    /* suspecting it's generally cheaper to jump backwards due to encoding,
     * the cutoff is unclear until we benchmark it, so the number chosen here
     * is arbitrary for now
     */
    if (idx * 3 < nentry) {
        *ze = _ziplist_fromleft(zl, idx);
    } else {
        *ze = _ziplist_fromright(zl, nentry - 1 - idx);
    }

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_find(zipentry_p *ze, int64_t *idx, const ziplist_p zl, const struct blob *val)
{
    uint32_t nentry, len;
    int64_t i;
    zipentry_p z;
    uint8_t *end;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }
    if (ze == NULL && idx == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    /* Encoding one struct blob and follow up with many simple memcmp should be
     * faster than decoding each of the zentries being compared.
     */
    len = _zipentry_encode(ze_buf, val);
    end = _ziplist_end(zl);
    for (i = 0, z =_ziplist_head(zl); i < nentry; i++, z = _ziplist_next(z)) {
        if (memcmp(z, ze_buf, MIN(end - z + 1, len)) == 0) { /* found */
            if (idx != NULL) {
                *idx = i;
            }
            if (ze != NULL) {
                *ze = z;
            }

            return ZIPLIST_OK;
        }
    }

    /* not found */
    if (idx != NULL) {
        *idx = -1;
    }
    if (ze != NULL) {
        *ze = NULL;
    }
    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_remove(ziplist_p zl, int64_t idx, uint32_t count)
{
    uint32_t nentry, i = 0;
    zipentry_p begin, end;

    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    if (count == 0) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx > nentry || nentry <= idx + count - 1) {
        return ZIPLIST_EOOB;
    }

    ziplist_locate(&begin, zl, idx);

    for (end = begin; i < count; ++i, end += _zipentry_len(end));
    cc_memmove(begin, end, _ziplist_end(zl) + 1 - end);

    ZL_NENTRY(zl) -= count;
    ZL_NEND(zl) -= (end - begin);

    return ZIPLIST_OK;
}

static inline void
_ziplist_add(ziplist_p zl, zipentry_p ze, struct blob *val)
{
    uint8_t sz;

    sz = _zipentry_encode(ze, val);

    ZL_NENTRY(zl) += 1;
    ZL_NEND(zl) += sz;
}

ziplist_rstatus_e
ziplist_insert(ziplist_p zl, struct blob *val, int64_t idx)
{
    uint32_t nentry;
    uint8_t sz;
    zipentry_p ze;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN) {
        return ZIPLIST_EINVALID;
    }

    nentry = ziplist_nentry(zl);
    idx += (idx < 0) * nentry;
    if (idx < 0 || idx > nentry) {
        return ZIPLIST_EOOB;
    }

    if (idx == nentry) {
        ze = _ziplist_end(zl) + 1;
    } else {
        sz = _encode_size(val);
        ziplist_locate(&ze, zl, idx);
        /* right shift */
        cc_memmove(ze + sz, ze, _ziplist_end(zl) - ze + 1);
    }

    _ziplist_add(zl, ze, val);

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_push(ziplist_p zl, struct blob *val)
{
    zipentry_p ze;

    if (zl == NULL || val == NULL) {
        return ZIPLIST_ERROR;
    }

    if (val->type == BLOB_TYPE_STR && val->vstr.len > ZE_STR_MAXLEN) {
        return ZIPLIST_EINVALID;
    }

    ze = _ziplist_end(zl) + 1;
    _ziplist_add(zl, ze, val);

    return ZIPLIST_OK;
}

ziplist_rstatus_e
ziplist_pop(struct blob *val, ziplist_p zl)
{
    zipentry_p ze;
    uint8_t *end;

    if (zl == NULL) {
        return ZIPLIST_ERROR;
    }

    if (ziplist_nentry(zl) == 0) {
        return ZIPLIST_EOOB;
    }

    ze = _ziplist_fromright(zl, 0);
    end = _ziplist_end(zl);
    if (val != NULL) {
        zipentry_get(val, ze); /* won't fail */
    }
    ZL_NENTRY(zl) -= 1;
    ZL_NEND(zl) -= *end;

    return ZIPLIST_OK;
}
