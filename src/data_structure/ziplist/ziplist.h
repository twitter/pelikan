#pragma once

/* reference: https://github.com/antirez/redis/blob/unstable/src/ziplist.h
 *
 * This is intended to be used in conjunction with the storage modules in
 * Pelikan, such as slab-based or cuckoo hashing based storage. Therefore,
 * we are making some notably different assumptions:
 *   - no memory allocation or freeing is attempted within this module
 *   - # of entries (nentry) is a 32-bit field, instead of 16, the 2 extra bytes
 *     allows us to guarantee O(1) runtime to get the cardinality of any ziplist
 *   - APIs are updated to reflect the changes above and Pelikan's style guide
 *   - no support for big-endian machines for the moment
 *   - unsigned integer only (also, redis' implementation is confusing to me)
 *   - optimize encoding for small integers not for small strings
 *   - do not allow anything longer than 252 byte to be stored as ziplist, in
 *       the future, such support can be added by using pointers to link to
 *       other items, something that worked in our earlier prototype.
 */

/* The description of the ziplist is adapted from the redis project with heavy
 * modification. License is included for comments only.
 * The binary does not contain any source code from Redis.
 */


/* The ziplist is a specially encoded dually linked list that is designed
 * to be very memory efficient. It stores both strings and integer values.
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
 * We also don't attempt to embed large values as ziplist entries, because the
 * operations on large values generally have very different considerations from
 * small ones. For example, it is much more important to make sure memory
 * operations are efficient (such as resizing and copying) when updating large
 * values, and the overhead of encoding becomes negligible. They also will have
 * very different runtime characteristics. So instead of supporting all value
 * sizes in theory and running into operational issues later, it is better,
 * at least operationally, to make such limitations explicit and deal with
 * different use cases separately.
 *
 * RUNTIME
 * =======
 * All lookups generally gets more expensive as the number of entries increases,
 * roughly linearlly (not considering various cache size cutoffs). For the same
 * list, looking up entries at the beginning are generally cheaper than entries
 * in the middle; for index-based lookup, both ends are cheaper than somewhere
 * in the middle, but for value-based lookup (where a match is performed), it
 * is more expensive if a match is found toward the end.
 *
 * Insertion and removal of entries involve index-based lookup, as well as
 * shifting data. So in additional to the considerations above, the amount of
 * data being moved for updates will affect performance. Updates near the "fixed
 * end" of the ziplist (currently the beginning) require moving more data and
 * therefore will be slower. Overall, it is cheapest to perform updates at the
 * end of the list due to minimal lookup cost and zero data movement.
 *
 * TODO: optimization if all list members are of the same size, then we can
 * remove the entry header all together and seeking will be extremely easy.
 *
 * TODO: if it is common to insert data at the beginning (e.g. a FIFO queue),
 * create additional heuristics or switches, similar to the ones used for
 * append/prepend in Twemcache (pelikan-twemcache), to reduce the amount of
 * data that need to be moved during insertion.
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

#include "../shared.h"

#define ZIPLIST_HEADER_SIZE 8

#define ZE_ZELEN_LEN  1
#define ZE_U8_MAX     250
#define ZE_U8_LEN     1
#define ZE_U16_MAX    UINT16_MAX
#define ZE_U16        251
#define ZE_U16_LEN    3
#define ZE_U24_MAX    ((1ULL << 24) - 1)
#define ZE_U24        252
#define ZE_U24_LEN    4
#define ZE_U56_MAX    ((1ULL << 56) - 1)
#define ZE_U56        253
#define ZE_U56_LEN    8
#define ZE_U64_MAX    UINT64_MAX
#define ZE_U64        254
#define ZE_U64_LEN    9
#define ZE_STR        255
#define ZE_STR_HEADER 2
#define ZE_STR_MAXLEN (UINT8_MAX - ZE_STR_HEADER - ZE_ZELEN_LEN)

#define ZL_NENTRY(_zl)  (*((uint32_t *)(_zl)))
#define ZL_NEND(_zl)  (*((uint32_t *)(_zl) + 1)) /* offset of last byte in zl */

typedef uint8_t * ziplist_p;
typedef uint8_t * zipentry_p;

typedef enum {
    ZIPLIST_OK,
    ZIPLIST_ENOTFOUND,  /* value not found error */
    ZIPLIST_EOOB,       /* out-of-bound error */
    ZIPLIST_EINVALID,   /* invalid data error */
    ZIPLIST_ERROR,
    ZIPLIST_SENTINEL
} ziplist_rstatus_e;

/* zipentry APIs */
ziplist_rstatus_e zipentry_size(uint8_t *sz, struct blob *val);
ziplist_rstatus_e zipentry_get(struct blob *val, const zipentry_p ze);
ziplist_rstatus_e zipentry_set(zipentry_p ze, const struct blob *val);
/* normal string or int comparison if both are of the same type, comparing an
 * int to an str will always return -1, and 1 vice versa */
int zipentry_compare(const zipentry_p ze, const struct blob *val);

/* ziplist APIs: seek */
ziplist_rstatus_e ziplist_prev(zipentry_p *ze, const ziplist_p zl, const zipentry_p curr);
ziplist_rstatus_e ziplist_next(zipentry_p *ze, const ziplist_p zl, const zipentry_p curr);
ziplist_rstatus_e ziplist_locate(zipentry_p *ze, const ziplist_p zl, int64_t idx);
/* return first match, entry & index, ze & idx can't both be NULL, idx is set to
 * -1 and ze is set to NULL if a match is not found */
ziplist_rstatus_e ziplist_find(zipentry_p *ze, int64_t *idx, const ziplist_p zl, const struct blob *val);

/* ziplist APIs: modify */
ziplist_rstatus_e ziplist_reset(ziplist_p zl);
/* remove `count' entries starting from index idx
 * a negative idx means the offset is from the end (last entry == -1);
 * a netaive count means deleting forward
 * count cannot be 0
 */
ziplist_rstatus_e ziplist_remove(ziplist_p zl, int64_t idx, int64_t count);
/* remove val (up to `count' occurrences), 0 for all, a negative count means
 * starting from the end
 */
ziplist_rstatus_e ziplist_remove_val(ziplist_p zl, struct blob *val, int64_t count);
/* if idx == nentry, value will be right pushed;
 * otherwise, existing entry is right shifted
 * CALLER MUST MAKE SURE THERE IS ENOUGH MEMORY!!!
 */
ziplist_rstatus_e ziplist_insert(ziplist_p zl, struct blob *val, int64_t idx);
ziplist_rstatus_e ziplist_push(ziplist_p zl, struct blob *val); /* a shorthand for insert at idx == nentry */
/* remove tail & return, if val is NULL it is equivalent to remove at idx -1 */
ziplist_rstatus_e ziplist_pop(struct blob *val, ziplist_p zl);

static inline uint32_t
ziplist_nentry(const ziplist_p zl)
{
    return ZL_NENTRY(zl);
}

static inline uint32_t
ziplist_size(const ziplist_p zl)
{
    return ZL_NEND(zl) + 1;
}
