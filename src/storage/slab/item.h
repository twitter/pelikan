#pragma once

#include <storage/slab/slab.h>

#include <time/time.h>

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_queue.h>

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/*          name                type            description */
#define ITEM_METRIC(ACTION)                                                             \
    ACTION( item_keyval_byte,   METRIC_GAUGE,        "# current item key + data bytes" )\
    ACTION( item_val_byte,      METRIC_GAUGE,        "# current data bytes"            )\
    ACTION( item_curr,          METRIC_GAUGE,        "# current items"                 )\
    ACTION( item_req,           METRIC_COUNTER,      "# items allocated"               )\
    ACTION( item_req_ex,        METRIC_COUNTER,      "# item alloc errors"             )\
    ACTION( item_remove,        METRIC_COUNTER,      "# items removed"                 )\
    ACTION( item_link,          METRIC_COUNTER,      "# items linked"                  )\
    ACTION( item_unlink,        METRIC_COUNTER,      "# items unlinked"                )

typedef struct item_metric {
    ITEM_METRIC(METRIC_DECLARE)
} item_metrics_st;

#define ITEM_METRIC_INIT(_metrics) do {                              \
    *(_metrics) = (item_metrics_st) { ITEM_METRIC(METRIC_INIT) }; \
} while(0)

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
    uint32_t          has_cas:1;     /* item has cas */
    uint32_t          in_freeq:1;    /* item in free queue */
    uint32_t          is_raligned:1; /* item data (payload) is right-aligned */
    uint32_t          vtype:1;       /* extra flag for type indication */
    uint32_t          vlen:27;       /* data size (27 bits since uint32_t is 32 bits and we have 5 flags)
                                        NOTE: need at least enough bits to support the largest value size allowed
                                        by the implementation, i.e. SLAB_MAX_SIZE */

    uint32_t          offset;        /* offset of item in slab */
    uint16_t          refcount;      /* # concurrent users of item */
    uint8_t           id;            /* slab class id */
    uint8_t           klen;          /* key length */
    char              end[1];        /* item data */
};

typedef enum {
    V_STR = 0,
    V_INT = 1,
} vtype_e;

#define ITEM_MAGIC      0xfeedface
#define ITEM_HDR_SIZE   offsetof(struct item, end)
#define ITEM_CAS_SIZE   sizeof(uint64_t)

#if __GNUC__ >= 4 && __GNUC_MINOR__ >= 2
#pragma GCC diagnostic ignored "-Wstrict-aliasing"
#endif

typedef enum item_rstatus {
    ITEM_OK,
    ITEM_ENOTFOUND,
    ITEM_EOVERSIZED,
    ITEM_ENOMEM,
    ITEM_EOTHER,
} item_rstatus_t;

static inline uint32_t
item_flag(struct item *it)
{
    return 0;
}

static inline uint64_t
item_get_cas(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    if (it->has_cas) {
        return *((uint64_t *)it->end);
    }

    return 0;
}

static inline void
item_set_cas(struct item *it, uint64_t cas)
{
    ASSERT(it->magic == ITEM_MAGIC);

    if (it->has_cas) {
        *((uint64_t *)it->end) = cas;
    }
}

#if __GNUC__ >= 4 && __GNUC_MINOR__ >= 6
#pragma GCC diagnostic pop
#endif

static inline char *
item_key(struct item *it)
{
    char *key;

    ASSERT(it->magic == ITEM_MAGIC);

    key = it->end;
    if (it->has_cas) {
        key += ITEM_CAS_SIZE;
    }

    return key;
}

static inline size_t
item_ntotal(uint8_t klen, uint32_t vlen, bool cas)
{
    size_t ntotal;

    ntotal = cas ? ITEM_CAS_SIZE : 0;
    ntotal += ITEM_HDR_SIZE + klen + vlen;

    return ntotal;
}

static inline size_t
item_size(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    return item_ntotal(it->klen, it->vlen, it->has_cas);
}

/* Set up/tear down the item module */
rstatus_t item_setup(uint32_t hash_power, item_metrics_st *metrics);
void item_teardown(void);

/* Get item payload */
char * item_data(struct item *it);

/* Get the slab the item belongs to */
struct slab *item_to_slab(struct item *it);

/* Init header for given item */
void item_hdr_init(struct item *it, uint32_t offset, uint8_t id);

/* Calculate slab id that will accommodate item with given key/val lengths */
uint8_t item_slabid(uint8_t klen, uint32_t vlen);

/* Allocate a new item */
item_rstatus_t item_alloc(struct item **it, const struct bstring *key, rel_time_t expire_at, uint32_t vlen);

/* Make an item with zero refcount available for reuse by unlinking it from hash */
void item_reuse(struct item *it);

/* Item lookup */
struct item *item_get(const struct bstring *key);

/* Set item value. Adds new item if item with key doesn't exist */
item_rstatus_t item_set(const struct bstring *key, const struct bstring *val, rel_time_t expire_at);

/* Perform check-and-set */
item_rstatus_t item_cas(const struct bstring *key, const struct bstring *val, rel_time_t expire_at, uint64_t cas);

/* Append/prepend */
item_rstatus_t item_annex(const struct bstring *key, const struct bstring *val, bool append);

/* In place item update (replace item value) */
item_rstatus_t item_update(struct item *it, const struct bstring *val);

/* Remove item from cache */
item_rstatus_t item_delete(const struct bstring *key);

/* flush the cache */
void item_flush(void);
