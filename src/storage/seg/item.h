#pragma once

#include "constant.h"

#include <cc_bstring.h>
#include <cc_metric.h>
#include <cc_queue.h>
#include <time/time.h>

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>


typedef enum item_rstatus {
    ITEM_OK,
    ITEM_EOVERSIZED,
    ITEM_ENOMEM,
    ITEM_ENAN, /* not a number */
    ITEM_EOTHER,
} item_rstatus_e;


/*
 * Every item chunk in the cache starts with a header (struct item)
 * followed by item data. An item is essentially a chunk of memory
 * carved out of a slab. Every item is owned by its parent slab.
 *
 * Items are either linked or unlinked. When item is first allocated and
 * has no data, it is unlinked. When data is copied into an item, it is
 * linked into the hash table (ITEM_LINKED). When item is deleted either
 * explicitly or due to item expiration, it is moved in the free q
 * (ITEM_FREEQ). The flags ITEM_LINKED and ITEM_FREEQ are mutually
 * exclusive and when an item is unlinked it has neither of these flags.
 *
 *   <-----------------------item size------------------>
 *   +---------------+----------------------------------+
 *   |               |                                  |
 *   |  item header  |          item payload            |
 *   | (struct item) |         ...      ...             |
 *   +---------------+-------+-------+------------------+
 *   ^               ^       ^       ^
 *   |               |       |       |
 *   |               |       |       |
 *   |               |       |       |
 *   |               |       |       \
 *   |               |       |       item_data()
 *   |               |       \
 *   \               |       item_key()
 *   item            \
 *                   item->end
 *
 * item->end is followed by:
 * - other optional metadata
 * - key
 * - data
 */

/* TODO(jason): we may want to change to open addressing hashtable to
 * save the 8-byte hash pointer,
 *
 * TODO(jason): if we remove hash_next, we can reduce memory alignment to 4-byte
 * but this still requires val to be uint32_t
 * TODO(jason): consider using vlen as part of val when is_num
 * */

struct item {
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    uint32_t magic; /* item magic (const) */
#endif

//    uint32_t seg_id : 24; /* id of the segment the item belongs to, max 16 TiB.
//                           * we can make this field optional if needed,
//                           * we don't need this field if we used a fixed  segment
//                           * size and only use DRAM/PMem for storage,
//                           * in such scenario, we can make the starting address
//                           * of each segment as multiple of 1<<20,
//                           * however, doing so adds several limitations,
//                           * and does not save space if we have cas as uint64_t
//                           * due to struct packing
//                           */

    uint32_t klen : 8; /* key size */
    uint32_t vlen : 24; /* data size */
    uint8_t is_num : 1; /* whether this is a number */

    uint8_t olen : 7; /* option length */

    /* data start needs to be 8-byte aligned for incr/decr */
    char end[1]; /* item data */
};

SLIST_HEAD(item_slh, item);

extern struct locktable cas_table;


/* get key length */
static inline uint32_t
item_nkey(const struct item *const it)
{
    return it->klen;
}

/*
 * because incr/decr does not change ttl, we need to do in-place update,
 * however, if original item is only 1 byte (such as "1"), then we cannot incr
 * over 9 in place, to solve this, we require vlen to be at least 4 bytes, so
 * that when we receive incr, we convert the value to uint32_t and mark in item,
 * then we treat the value as uint32_t in the future
 */
static inline uint32_t
item_nval(const struct item *const it)
{
    return it->vlen > sizeof(uint64_t) ? it->vlen : sizeof(uint64_t);
}

static inline uint32_t
item_nopt(const struct item *const it)
{
    return it->olen;
}

static inline uint32_t
item_olen(const struct item *const it)
{
    return it->olen;
}

static inline char *
item_optional(struct item *const it)
{
    if (it->olen != 0) {
        return it->end;
    } else {
        return NULL;
    }
}

static inline char *
item_key(struct item *const it)
{
    return it->end + item_olen(it);
}

/*
 * Get start location of item value
 */
static inline char *
item_val(struct item *const it)
{
    return it->end + item_nkey(it) + item_olen(it);
}

/*
 * round up total occupied size in DRAM for memalign
 */
static inline size_t
item_size_roundup(const uint32_t sz)
{
    return (((sz - 1) >> 3u) + 1u) << 3u;
}

static inline size_t
item_size(uint32_t klen, uint32_t vlen, uint32_t olen)
{
    size_t sz = ITEM_HDR_SIZE + klen + olen;
    sz += vlen > sizeof(uint64_t) ? vlen : sizeof(uint64_t);
    return item_size_roundup(sz);
}

static inline size_t
item_ntotal(const struct item *it)
{
    size_t sz = ITEM_HDR_SIZE + it->klen + it->olen + item_nval(it);

    /* we need to make sure memory is aligned at 8-byte boundary */
    return item_size_roundup(sz);
}


item_rstatus_e
item_incr(uint64_t *vint, struct item *it, uint64_t delta);

item_rstatus_e
item_decr(uint64_t *vint, struct item *it, uint64_t delta);

/* this is for incr/decr,
 * after we parse the val with item_atou32, we set the new value with this
 * func */
// static inline void
// item_set_val_uint32(struct item *it, uint32_t vint)
//{
//    ASSERT(it->is_num);
//    *(uint32_t *)item_val(it) = vint;
//}

void
item_release(struct item *it);


/* acquire an item */
struct item *
item_get(const struct bstring *key, uint64_t *cas, bool incr_ref);

/* this function does insert or update */
void
item_insert(struct item *it);


/* reserve an item, this does not link it or remove existing item with the same
 * key.
 * olen- optional data length, this can be used to reserve space for optional
 * data, e.g. flag in Memcached protocol) in payload, after cas.
 * */
item_rstatus_e
item_reserve(struct item **it_p, const struct bstring *key,
        const struct bstring *val, uint32_t vlen, uint8_t olen,
        proc_time_i expire_at);

void
item_backfill(struct item *it, const struct bstring *val);

/* replace the item in the hashtable with given item */
void
item_update(struct item *it);

/* Remove item from cache */
bool
item_delete(const struct bstring *key);

///* remove this specific item from hashtable */
//bool
//item_evict(const char *oit_key, const uint32_t oit_klen,
//               const uint32_t seg_id, const uint32_t offset);

///* remove oit from hashtable and insert nit */
//void
//item_relink(struct item *oit, struct item *nit);


/* flush the cache */
void
item_flush(void);

