#pragma once

#include <storage/slab/item.h>

struct hash_table {
    struct item_slh *table;
    uint32_t nhash_item;
    uint32_t hash_power;
};

#define HASHSIZE(_n) (1UL << (_n))
#define HASHMASK(_n) (HASHSIZE(_n) - 1)

struct hash_table *hashtable_create(uint32_t hash_power);
void hashtable_destroy(struct hash_table *table);

void hashtable_put(struct item *it, struct hash_table *table);
void hashtable_delete(const uint8_t *key, uint32_t klen, struct hash_table *table);
struct item *hashtable_get(const uint8_t *key, uint32_t klen, struct hash_table *table);
