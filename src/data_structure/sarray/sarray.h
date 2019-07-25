#pragma once

/* The sarray (sorted array) is designed for sorted array of integers of uniform
 * but configurable sizes. Currently we only implemented this for unsigned
 * integer entries of width 1-, 2-, 4-, 8-byte. But it can be extended to byte
 * strings as well. The array is stored in ASC order without duplicates. Once an
 * array is created, these configurable attributes cannot be changed.
 *
 * Because of the limitation on data type, sarray is both more memory-
 * efficient and faster for value lookups compared to a more generic data
 * structure such as ziplist. It is particularly useful if users intend to
 * keep a sorted list of numbers without duplication, such as an index of
 * numeric IDs.
 *
 * ----------------------------------------------------------------------------
 *
 * SARRAY OVERALL LAYOUT
 * =====================
 *
 * The general layout of the sarray is as follows:
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
 * SARRAY ENTRIES
 * ==============
 *
 * Every entry in the sarray is a simple integer of size specified in the
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

#define SARRAY_HEADER_SIZE 8

typedef uint8_t * sarray_p;

typedef enum {
    SARRAY_OK,
    SARRAY_ENOTFOUND,  /* value not found error */
    SARRAY_EOOB,       /* out-of-bound error */
    SARRAY_EINVALID,   /* invalid data error */
    SARRAY_EDUP,       /* duplicate value found */
    SARRAY_ERROR,
    SARRAY_SENTINEL
} sarray_rstatus_e;

#define SA_NENTRY(_sa) (*((uint32_t *)(_sa)))
#define SA_ESIZE(_sa) (*((uint32_t *)(_sa) + 1))

static inline uint32_t
sarray_nentry(const sarray_p sa)
{
    return SA_NENTRY(sa);
}

static inline uint32_t
sarray_esize(const sarray_p sa)
{
    return SA_ESIZE(sa);
}

/* initialize an sarray of element size 1/2/4/8 bytes */
sarray_rstatus_e sarray_init(sarray_p sa, uint32_t esize);

/* sarray APIs: seek */
sarray_rstatus_e sarray_value(uint64_t *val, const sarray_p sa, uint32_t idx);
sarray_rstatus_e sarray_index(uint32_t *idx, const sarray_p sa, uint64_t val);

/* ziplist APIs: modify */
sarray_rstatus_e sarray_insert(sarray_p sa, uint64_t val);
sarray_rstatus_e sarray_remove(sarray_p sa, uint64_t val);

/*
 * if count is positive, remove count entries starting at the beginning
 * if count is negative, remove -count entries starting at the end
 */
sarray_rstatus_e sarray_truncate(sarray_p sa, int64_t count);

