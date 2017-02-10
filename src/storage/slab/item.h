#pragma once

#include "slabclass.h"

#include "time/time.h"

#include <cc_bstring.h>
#include <cc_metric.h>
#include <cc_queue.h>

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

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
 *                   item->end, (if enabled) item_get_cas()
 *
 * item->end is followed by:
 * - 8-byte cas, if ITEM_CAS flag is set
 * - key
 * - data
 */
struct item {
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    uint32_t          magic;         /* item magic (const) */
#endif
    SLIST_ENTRY(item) i_sle;         /* link in hash/freeq */
    rel_time_t        expire_at;     /* expiry time in secs */
    rel_time_t        create_at;     /* time when this item was last linked */

    uint32_t          is_linked:1;   /* item in hash */
    uint32_t          in_freeq:1;    /* item in free queue */
    uint32_t          is_raligned:1; /* item data (payload) is right-aligned */
    uint32_t          vlen:29;       /* data size (29 bits since uint32_t is 32 bits and we have 3 flags)
                                        NOTE: need at least enough bits to support the largest value size allowed
                                        by the implementation, i.e. SLAB_MAX_SIZE */

    uint32_t          offset;        /* offset of item in slab */
    uint32_t          dataflag;      /* data flags opaque to the server */
    uint8_t           id;            /* slab class id */
    uint8_t           klen;          /* key length */
    uint16_t          padding;       /* keep end 64-bit aligned, it may be a cas */
    char              end[1];        /* item data */
};
/* TODO(yao): dataflag is memcached-specific, can we abstract it out of storage? */

#define ITEM_MAGIC      0xfeedface
#define ITEM_HDR_SIZE   offsetof(struct item, end)
#define ITEM_CAS_SIZE   sizeof(uint64_t)

#if __GNUC__ >= 4 && __GNUC_MINOR__ >= 2
#pragma GCC diagnostic ignored "-Wstrict-aliasing"
#endif

typedef enum item_rstatus {
    ITEM_OK,
    ITEM_EOVERSIZED,
    ITEM_ENOMEM,
    ITEM_ENAN, /* not a number */
    ITEM_EOTHER,
} item_rstatus_t;

extern bool use_cas;
extern uint64_t cas_id;

static inline uint32_t
item_flag(struct item *it)
{
    return it->dataflag;
}

static inline uint64_t
item_get_cas(struct item *it)
{
    return use_cas ? *((uint64_t *)it->end) : 0;
}

static inline void
item_set_cas(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    if (use_cas) {
        *((uint64_t *)it->end) = ++cas_id;
    }
}

static inline size_t
item_cas_size(void)
{
    return use_cas * ITEM_CAS_SIZE;
}

#if __GNUC__ >= 4 && __GNUC_MINOR__ >= 6
#pragma GCC diagnostic pop
#endif

static inline char *
item_key(struct item *it)
{
    return it->end + item_cas_size();
}

static inline size_t
item_ntotal(uint8_t klen, uint32_t vlen)
{
    return ITEM_HDR_SIZE + item_cas_size() + klen + vlen;
}

static inline size_t
item_size(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    return item_ntotal(it->klen, it->vlen);
}

/*
 * Get start location of item payload
 */
static inline char *
item_data(struct item *it)
{
    char *data;

    if (it->is_raligned) {
        data = (char *)it + slabclass[it->id].size - it->vlen;
    } else {
        data = it->end + item_cas_size() + it->klen;
    }

    return data;
}

static inline item_rstatus_t
item_atou64(uint64_t *vint, struct item *it)
{
    rstatus_i status;
    struct bstring vstr;

    vstr.len = it->vlen;
    vstr.data = (char *)item_data(it);
    status = bstring_atou64(vint, &vstr);
    if (status == CC_OK) {
        return ITEM_OK;
    } else {
        return ITEM_ENAN;
    }
}

/* Init header for given item */
void item_hdr_init(struct item *it, uint32_t offset, uint8_t id);

/* acquire an item */
struct item *item_get(const struct bstring *key);

/* TODO: make the following APIs protocol agnostic */

/* insert an item, removes existing item of the same key (if applicable) */
void item_insert(struct item *it, const struct bstring *key);

/* reserve an item, this does not link it or remove existing item with the same key */
item_rstatus_t item_reserve(struct item **it_p, const struct bstring *key, const struct bstring *val, uint32_t vlen, uint32_t dataflag, rel_time_t expire_at);
/* item_release is used for reserved item only (not linked) */
void item_release(struct item **it_p);

void item_backfill(struct item *it, const struct bstring *val);

/* Append/prepend */
item_rstatus_t item_annex(struct item *it, const struct bstring *key, const struct bstring *val, bool append);


/* In place item update (replace item value) */
void item_update(struct item *it, const struct bstring *val);

/* Remove item from cache */
bool item_delete(const struct bstring *key);

/* flush the cache */
void item_flush(void);
