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

#include "../shared.h"

#include <stdint.h>

typedef uint8_t * ziplist_p;
typedef uint8_t * zipentry_p;

typedef enum {
    ZIPLIST_OK,
    ZIPLIST_EOOB,       /* out-of-bound error */
    ZIPLIST_EINVALID,   /* invalid data error */
    ZIPLIST_ERROR,
    ZIPLIST_SENTINEL
} ziplist_rstatus_e;

/* zipentry APIs */
ziplist_rstatus_e zipentry_get(struct blob *val, const zipentry_p ze);
ziplist_rstatus_e zipentry_set(zipentry_p ze, const struct blob *val);
/* normal string or int comparison if both are of the same type, comparing an int
 * to an str will always return -1, and 1 vice versa */
int zipentry_compare(const zipentry_p ze, const struct blob *val);

/* ziplist APIs: seek */
ziplist_rstatus_e ziplist_prev(zipentry_p *ze, const ziplist_p zl, const zipentry_p curr);
ziplist_rstatus_e ziplist_next(zipentry_p *ze, const ziplist_p zl, const zipentry_p curr);
ziplist_rstatus_e ziplist_locate(zipentry_p *ze, const ziplist_p zl, uint32_t idx);
ziplist_rstatus_e ziplist_find(zipentry_p *ze, const ziplist_p zl, const struct blob *val); /* first match */

/* ziplist APIs: modify */
ziplist_rstatus_e ziplist_reset(ziplist_p zl);
ziplist_rstatus_e ziplist_remove(const ziplist_p zl, uint32_t idx, uint32_t count); /* count entries starting from index idx */
/* if idx == nentry, value will be right pushed;
 * otherwise, existing entry is right shifted
 * CALLER MUST MAKE SURE THERE IS ENOUGH MEMORY!!!
 */
ziplist_rstatus_e ziplist_insert(ziplist_p zl, struct blob *val, uint32_t idx);

static inline uint32_t
ziplist_nentry(const ziplist_p zl)
{
    return *((uint32_t *)zl);
}

static inline uint32_t
ziplist_len(const ziplist_p zl)
{
    return *((uint32_t *)zl + 1) + 1;
}

