#ifndef _BB_ASSOC_H_
#define _BB_ASSOC_H_

#include <storage/slab/bb_item.h>

struct hash_table {
    struct item_slh *table;
    uint32_t nhash_item;
    uint32_t hash_power;
};

struct hash_table *assoc_create(uint32_t hash_power);
void assoc_destroy(struct hash_table *table);

void assoc_put(struct item *it, struct hash_table *table);
void assoc_delete(const uint8_t *key, uint32_t klen, struct hash_table *table);
struct item *assoc_get(const uint8_t *key, uint32_t klen, struct hash_table *table);

#endif /* _BB_ASSOC_H_ */
