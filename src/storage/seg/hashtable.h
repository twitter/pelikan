#pragma once

#include "item.h"

struct hash_table {
    struct item_slh *table;
    uint32_t nhash_item;
    uint32_t hash_power;
};

//#define HASHSIZE(_n) (1ULL << (_n))
//#define HASHMASK(_n) (HASHSIZE(_n) - 1)

struct hash_table *
hashtable_create(uint32_t hash_power);
void
hashtable_destroy(struct hash_table **ht_p);

void
hashtable_put(struct item *it, struct hash_table *ht);

/* try to delete the item from hashtable, if try_del is false,
 * the item must be in the hashtable
 * stored the ptr to the deleted item in it, this is safe because
 * the item has not been wiped from segment */
bool
hashtable_delete(const char *key, uint32_t klen, struct hash_table *ht,
        bool try_del, struct item **it);

/*
 * delete the hashtable entry only if item is the up-to-date/valid item
 */
bool
hashtable_delete_it(struct item *oit, struct hash_table *ht);

struct item *
hashtable_get(const char *key, uint32_t klen, struct hash_table *ht);


struct hash_table *
hashtable_double(struct hash_table *ht); /* best effort expansion */
