#pragma once

/* This is an implementation of hashmaps with bounded but flexible entry size
 * with binary field keys. The size of both field key and value are limited to
 * 255 bytes in this POC.
 *
 * The fields are sorted but not indexed. This makes bulk lookup faster when the
 * field (keys) are also sorted.
 *
 * ----------------------------------------------------------------------------
 *
 * HASHMAP OVERALL LAYOUT
 * =====================
 *
 * The general layout of the hashmap is as follows:
 *                         entry
 *                ╭------------------------╮
 * <nentry><nbody><eklen><evlen><ekey><eval> ... <eklen><evlen><ekey><eval>
 * ╰-------------╯╰-------------------------------------------------------╯
 *     header                             body
 *
 * Overhead: 8 bytes (two 32-bit integers)
 *
 * <uint32_t nentry> is the number of entries.
 * <uint32_t nbody> is the number of bytes in the body (not including header).
 *
 *
 *
 * HASHMAP ENTRIES
 * ==============
 *
 * For each entry:
 * <uint8_t eklen> is the size of hash field in each entry (entry key)
 * <uint8_t evlen> is the size of hash value in each entry (entry value)
 *
 * The rest of the entry is a tuple of a binary string (non-empty byte array)
 * for field and a byte array for value.
 *
 * RUNTIME
 * =======
 *
 * Entry lookup takes O(N) where N is the number of entries in the list.
 *
 * Insertion and removal of entries involve scan-based lookup, as well as
 * shifting data. So in additional to the considerations above, the amount of
 * data being moved for updates will affect performance. Updates near the "fixed
 * end" of the hashmap (currently the beginning) require moving more data and
 * therefore will be slower. Overall, it is cheapest to perform updates at the
 * end of the array due to zero data movement.
 *
 */

#include <cc_bstring.h>

#include <stdint.h>

#define HASHMAP_HEADER_SIZE (sizeof(uint32_t) + sizeof(uint32_t))      /* 8 */
#define HASHMAP_ENTRY_HEADER_SIZE (sizeof(uint8_t) + sizeof(uint8_t))  /* 2 */

typedef char * hashmap_p;

typedef enum {
    HASHMAP_OK,
    HASHMAP_ENOTFOUND,  /* value not found error */
    HASHMAP_EINVALID,   /* invalid (entry) data error */
    HASHMAP_EDUP,       /* duplicate entry found */
    HASHMAP_ERROR,
    HASHMAP_SENTINEL
} hashmap_rstatus_e;

#define HM_NENTRY(_hm) (*((uint32_t *)(_hm)))
#define HM_NBODY(_hm) (*((uint32_t *)((_hm) + sizeof(uint32_t))))

static inline uint32_t
hashmap_nentry(const hashmap_p hm)
{
    return HM_NENTRY(hm);
}

static inline uint32_t
hashmap_size(const hashmap_p hm)
{
    return HASHMAP_HEADER_SIZE + HM_NBODY(hm);
}

/* initialize an hashmap of key size 1/2/4/8 bytes and vsize */
hashmap_rstatus_e hashmap_init(hashmap_p hm);

/* hashmap APIs: seek */
hashmap_rstatus_e hashmap_get(struct bstring *val, const hashmap_p hm, const struct bstring *key);
uint32_t hashmap_multiget(struct bstring *val[], const hashmap_p hm, const struct bstring *key[], uint32_t cardinality);

/* hashmap APIs: modify */
hashmap_rstatus_e hashmap_insert(hashmap_p hm, const struct bstring *key, const struct bstring *val);
hashmap_rstatus_e hashmap_bulk_insert(hashmap_p hm, const struct bstring *key, const struct bstring *val);
hashmap_rstatus_e hashmap_remove(hashmap_p hm, const struct bstring *key);
hashmap_rstatus_e hashmap_bulk_insert(hashmap_p hm, const struct bstring *key, const struct bstring *val);
