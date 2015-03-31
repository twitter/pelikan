#include <storage/slab/bb_assoc.h>

#include <cc_hash.h>
#include <cc_mm.h>

#define HASH_DEFAULT_POWER 16

#define HASHSIZE(_n) (1UL << (_n))
#define HASHMASK(_n) (HASHSIZE(_n) - 1)

/*
 * Allocate table given size
 */
static struct item_slh *
assoc_alloc(uint32_t size)
{
    struct item_slh *table;
    uint32_t i;

    table = cc_alloc(sizeof(*table) * size);

    if(table != NULL) {
        for(i = 0; i < size; ++i) {
            SLIST_INIT(&table[i]);
        }
    }

    return table;
}

struct hash_table *
assoc_create(uint32_t hash_power)
{
    struct hash_table *table;
    uint32_t size;

    /* alloc struct */
    table = cc_alloc(sizeof(struct hash_table));

    if(table == NULL) {
        return NULL;
    }

    /* init members */
    table->table = NULL;
    table->hash_power = hash_power > 0 ? hash_power : HASH_DEFAULT_POWER;
    table->nhash_item = 0;
    size = HASHSIZE(table->hash_power);

    /* alloc table */
    table->table = assoc_alloc(size);

    if(table->table == NULL) {
        cc_free(table);
        return NULL;
    }

    return table;
}

rstatus_t
assoc_destroy(struct hash_table *table)
{
    if(table->table != NULL) {
        cc_free(table->table);
    }

    return CC_OK;
}

static struct item_slh *
assoc_get_bucket(const uint8_t *key, size_t klen, struct hash_table *table)
{
    return &(table->table[hash(key, klen, 0) & HASHMASK(table->hash_power)]);
}

void
assoc_put(struct item *it, struct hash_table *table)
{
    struct item_slh *bucket;

    ASSERT(assoc_get((uint8_t *)item_key(it), it->klen, table) == NULL);

    bucket = assoc_get_bucket((uint8_t *)item_key(it), it->klen, table);
    SLIST_INSERT_HEAD(bucket, it, i_sle);

    ++(table->nhash_item);
}

void
assoc_delete(const uint8_t *key, uint32_t klen, struct hash_table *table)
{
    struct item_slh *bucket;
    struct item *it, *prev;

    ASSERT(assoc_get(key, klen, table) != NULL);

    bucket = assoc_get_bucket(key, klen, table);
    for(prev = NULL, it = SLIST_FIRST(bucket); it != NULL;
        prev = it, it = SLIST_NEXT(it, i_sle)) {
        /* iterate through bucket to find item to be removed */
        if((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            break;
        }
    }

    if(prev == NULL) {
        SLIST_REMOVE_HEAD(bucket, i_sle);
    } else {
        SLIST_REMOVE_AFTER(prev, i_sle);
    }

    --(table->nhash_item);
}

struct item *
assoc_get(const uint8_t *key, uint32_t klen, struct hash_table *table)
{
    struct item_slh *bucket;
    struct item *it;

    ASSERT(key != NULL);
    ASSERT(klen != 0);

    bucket = assoc_get_bucket(key, klen, table);

    /* iterate through bucket looking for item */
    for(it = SLIST_FIRST(bucket); it != NULL; it = SLIST_NEXT(it, i_sle)) {
        if((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            return it;
        }
    }

    return NULL;
}
