#include "hashtable.h"

#include <hash/cc_murmur3.h>
#include <cc_mm.h>

static uint32_t murmur3_iv = 0x3ac5d673;

/*
 * Allocate table given size
 */
static struct item_slh *
_hashtable_alloc(uint64_t size)
{
    struct item_slh *table;
    uint32_t i;

    table = cc_alloc(sizeof(*table) * size);

    if (table != NULL) {
        for (i = 0; i < size; ++i) {
            SLIST_INIT(&table[i]);
        }
    }

    return table;
}

struct hash_table *
hashtable_create(uint32_t hash_power)
{
    struct hash_table *ht;
    uint64_t size;

    ASSERT(hash_power > 0);

    /* alloc struct */
    ht = cc_alloc(sizeof(struct hash_table));

    if (ht == NULL) {
        return NULL;
    }

    /* init members */
    ht->table = NULL;
    ht->hash_power = hash_power;
    ht->nhash_item = 0;
    size = HASHSIZE(ht->hash_power);

    /* alloc table */
    ht->table = _hashtable_alloc(size);
    if (ht->table == NULL) {
        cc_free(ht);
        return NULL;
    }

    return ht;
}

void
hashtable_destroy(struct hash_table *ht)
{
    if (ht != NULL && ht->table != NULL) {
        cc_free(ht->table);
    }
}

static struct item_slh *
_get_bucket(const char *key, size_t klen, struct hash_table *ht)
{
    uint32_t hv;

    hash_murmur3_32(key, klen, murmur3_iv, &hv);

    return &(ht->table[hv & HASHMASK(ht->hash_power)]);
}

void
hashtable_put(struct item *it, struct hash_table *ht)
{
    struct item_slh *bucket;

    ASSERT(hashtable_get(item_key(it), it->klen, ht) == NULL);

    bucket = _get_bucket(item_key(it), it->klen, ht);
    SLIST_INSERT_HEAD(bucket, it, i_sle);

    ++(ht->nhash_item);
}

void
hashtable_delete(const char *key, uint32_t klen, struct hash_table *ht)
{
    struct item_slh *bucket;
    struct item *it, *prev;

    ASSERT(hashtable_get(key, klen, ht) != NULL);

    bucket = _get_bucket(key, klen, ht);
    for (prev = NULL, it = SLIST_FIRST(bucket); it != NULL;
        prev = it, it = SLIST_NEXT(it, i_sle)) {
        /* iterate through bucket to find item to be removed */
        if ((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            break;
        }
    }

    if (prev == NULL) {
        SLIST_REMOVE_HEAD(bucket, i_sle);
    } else {
        SLIST_REMOVE_AFTER(prev, i_sle);
    }

    --(ht->nhash_item);
}

struct item *
hashtable_get(const char *key, uint32_t klen, struct hash_table *ht)
{
    struct item_slh *bucket;
    struct item *it;

    ASSERT(key != NULL);
    ASSERT(klen != 0);

    bucket = _get_bucket(key, klen, ht);
    /* iterate through bucket looking for item */
    for (it = SLIST_FIRST(bucket); it != NULL; it = SLIST_NEXT(it, i_sle)) {
        if ((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            return it;
        }
    }

    return NULL;
}
