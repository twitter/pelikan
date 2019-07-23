#pragma once

/* The intarray is designed for sorted array of integers of uniform but
 * configurable sizes, including 1-, 2-, 4-, 8-byte unsigned integers.
 * The array can be of ASC or DESC order. Once an array is created, these
 * configurable attributes cannot be changed.
 *
 * Because of the limitation on data type, intarray is both more memory-
 * efficient and faster for value lookups compared to a more generic data
 * structure such as ziplist. It is particularly useful if users intend to
 * keep a sorted list of numbers without duplication, such as an index of
 * numeric IDs.
 *
 * NOTE: start with ASC order, allow DESC later.
 *
 * ----------------------------------------------------------------------------
 *
 * INTARRAY OVERALL LAYOUT
 * =======================
 *
 * The general layout of the intarray is as follows:
 *
 * <nentry><esize> <entry> <entry> ... <entry>
 * ╰------------╯    ╰-------------------------╯
 *     header                   body
 *
 * Overhead: 8 bytes
 *
 * <uint32_t nentry> is the number of entries.
 *
 * <uint32_t esize> is the size of each entry (of value 1, 2, 4, 8 for now)
 *
 *
 * INTARRAY ENTRIES
 * ================
 *
 * Every entry in the intarray is a simple integer of size specified in the
 * header.
 *
 * RUNTIME
 * =======
 *
 * Entry lookup takes O(log N) where N is the number of entries in the list. If
 * the entry size are below a threshold (64-bytes for now), then a linear scan
 * is performed instead of binary lookup.
 *
 * Insertion and removal of entries involve index-based lookup, as well as
 * shifting data. So in additional to the considerations above, the amount of
 * data being moved for updates will affect performance. Updates near the "fixed
 * end" of the ziplist (currently the beginning) require moving more data and
 * therefore will be slower. Overall, it is cheapest to perform updates at the
 * end of the array due to zero data movement.
 *
 */

#include <stdint.h>

#define INTARRAY_HEADER_SIZE 8

typedef uint8_t * intarray_p;

typedef enum {
    INTARRAY_OK,
    INTARRAY_ENOTFOUND,  /* value not found error */
    INTARRAY_EOOB,       /* out-of-bound error */
    INTARRAY_EINVALID,   /* invalid data error */
    INTARRAY_EDUP,       /* duplicate value found */
    INTARRAY_ERROR,
    INTARRAY_SENTINEL
} intarray_rstatus_e;

#define IA_NENTRY(_ia) (*((uint32_t *)(_ia)))
#define IA_ESIZE(_ia) (*((uint32_t *)(_ia) + 1))

static inline uint32_t
intarray_nentry(const intarray_p ia)
{
    return IA_NENTRY(ia);
}

static inline uint32_t
intarray_esize(const intarray_p ia)
{
    return IA_ESIZE(ia);
}

intarray_rstatus_e intarray_init(intarray_p ia, uint32_t esize);

/* intarray APIs: seek */
intarray_rstatus_e intarray_value(uint64_t *val, const intarray_p ia, uint32_t idx);
intarray_rstatus_e intarray_index(uint32_t *idx, const intarray_p ia, uint64_t val);

/* ziplist APIs: modify */
intarray_rstatus_e intarray_insert(intarray_p ia, uint64_t val);
intarray_rstatus_e intarray_remove(intarray_p ia, uint64_t val);

/*
 * if count is positive, remove count entries starting at the beginning
 * if count is negative, remove -count entries starting at the end
 */
intarray_rstatus_e intarray_truncate(intarray_p ia, int64_t count);

