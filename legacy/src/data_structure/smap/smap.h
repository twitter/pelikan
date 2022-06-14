#pragma once

/* The smap (sorted map) is designed for maps with uniform entry size and keyed
 * on sorted integers. The size of both key and value are configurable upon
 * creation.
 * Currently we only implemented keys that are unsigned integers of width 1-,
 * 2-, 4-, 8-byte. But it can be extended to byte strings as well. The values
 * could be binary blobs of a fixed size up to 2**16 bytes. The map is stored in
 * ASC order without duplicates. Once an array is created, these configurable
 * attributes cannot be changed.
 *
 * Entry boundary is aligned based on the size of the key, i.e. if the key size
 * is 64-bit, all entries' start address are 64-bit aligned, if the key size is
 * 16-bit, entries are 16-bit aligned, etc. This is to ensure simple typecast of
 * pointers to keys would work. Otherwise, integers need to be first copied to
 * byte aligned address before typecasting for read, and `memcpy' has to be used
 * for write.
 *
 * TODO(yao): support variable size up to a max within the same map object
 *
 * Because of the limitation on data type, smap is both more memory-efficient
 * and faster for key lookups compared to a more generic data structure such as
 * ziplist. It is particularly useful if users intend to keep a sorted map of
 * entries without duplication, such as key-val pairs indexed by numeric IDs.
 *
 * ----------------------------------------------------------------------------
 *
 * SMAP OVERALL LAYOUT
 * =====================
 *
 * The general layout of the smap is as follows:
 *                          entry
 *                        ╭--------╮
 * <nentry><ksize><vsize> <key><val> <key><val> ... <key><val>
 * ╰--------------------╯ ╰----------------------------------╯
 *         header                         body
 *
 * Overhead: 8 bytes
 *
 * <uint32_t nentry> is the number of entries.
 *
 * For each entry:
 * <uint16_t ksize> is the size of key field in each entry (of value 1, 2, 4, 8
 *   for now)
 * <uint16_t vsize> is the size of val field in each entry
 *
 *
 * SMAP ENTRIES
 * ==============
 *
 * Every entry in the smap is a tuple of one integer and a byte array of sizes
 * specified in the header.
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

#include <cc_bstring.h>

#include <stdint.h>

#define SMAP_HEADER_SIZE 8

typedef char * smap_p;

typedef enum {
    SMAP_OK,
    SMAP_ENOTFOUND,  /* value not found error */
    SMAP_EOOB,       /* out-of-bound error */
    SMAP_EINVALID,   /* invalid data error */
    SMAP_EDUP,       /* duplicate value found */
    SMAP_ERROR,
    SMAP_SENTINEL
} smap_rstatus_e;

#define SM_NENTRY(_sm) (*((uint32_t *)(_sm)))
#define SM_KSIZE(_sm) (*((uint16_t *)((_sm) + sizeof(uint32_t))))
#define SM_VSIZE(_sm) (*((uint16_t *)((_sm) + sizeof(uint32_t) + sizeof(uint16_t))))

static inline uint32_t
smap_nentry(const smap_p sm)
{
    return SM_NENTRY(sm);
}

static inline uint16_t
smap_ksize(const smap_p sm)
{
    return SM_KSIZE(sm);
}

static inline uint16_t
smap_vsize(const smap_p sm)
{
    return SM_VSIZE(sm);
}

static inline uint32_t
smap_esize(const smap_p sm)
{
    uint32_t ksize = smap_ksize(sm);

    /* force alignment by key size */
    return ((ksize * 2 + smap_vsize(sm) - 1) / ksize) * ksize;
}

static inline uint32_t
smap_size(const smap_p sm)
{
    return SMAP_HEADER_SIZE + smap_esize(sm) * SM_NENTRY(sm);
}

/* initialize an smap of key size 1/2/4/8 bytes and vsize */
smap_rstatus_e smap_init(smap_p sm, uint16_t ksize, uint16_t vsize);

/* smap APIs: seek */
smap_rstatus_e smap_keyval(uint64_t *key, struct bstring *val, const smap_p sm, uint32_t idx);
smap_rstatus_e smap_index(uint32_t *idx, const smap_p sm, uint64_t key);

/* smap APIs: modify */
smap_rstatus_e smap_insert(smap_p sm, uint64_t key, const struct bstring *val);
smap_rstatus_e smap_remove(smap_p sm, uint64_t key);

/*
 * if count is positive, remove count entries starting at the beginning
 * if count is negative, remove -count entries starting at the end
 */
smap_rstatus_e smap_truncate(smap_p sm, int64_t count);
