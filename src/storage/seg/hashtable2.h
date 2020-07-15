#pragma once

#include "item.h"

#include <pthread.h>

/**
 * we use a bulk-chaining hash table,
 * each bucket is an array storing HASH_BUCKET_SIZE (fake) item pointers,
 * default HASH_BUCKET_SIZE = 8, which means one bucket stores 8 pointers,
 * if the number of items hash to one bucket exceeds 8, a new array of
 * size HASH_BUCKET_SIZE pointers are dynamically allocated and linked to
 * this bucket, see below for details
 *
 *
 *
 *
 *
 * impact of load factor on the number of arrays in each bucket (array size 8)
 * +=============+===========+============+===============+
 * | load factor | one array | two arrays | three arrays  |
 * +=============+===========+============+===============+
 * |         0.3 | 99.5%     | 0.5%       | 0             |
 * +-------------+-----------+------------+---------------+
 * |         0.6 | 89%       | 11%        | <0.01%        |
 * +-------------+-----------+------------+---------------+
 * |        0.93 | 54%       | 46%        | <1%           |
 * +-------------+-----------+------------+---------------+
 * |         1.2 | 24%       | 73%        | 3.5%          |
 * +-------------+-----------+------------+---------------+
 * NOTE: the nubmer of arrays depend on the array size, the table above is
 * calculated using array size 8, if we use larger array, then the corresponding
 * number will be smaller
 *
 *
 */

/* I hate to make seg_id, offset consistent, everywhere else is uint32_t,
 * here we use uint64_t to avoid conversion from uint32_t to uint64_t in
 * the function */

struct hash_table {
    uint32_t hash_power;
    uint64_t hash_mask;             /* avoid repeated computation*/
    uint64_t *table;
};



//#define HASHSIZE(_n) (1ULL << (_n))
//#define HASHMASK(_n) (HASHSIZE(_n) - 1)


void
hashtable_setup(uint32_t hash_power);
void
hashtable_teardown(void);


bool hashtable_del_and_put(struct item *it);

void
hashtable_put(struct item *it, const uint64_t seg_id, const uint64_t offset);

/* try to delete the item from hashtable, if try_del is false,
 * the item must be in the hashtable
 * stored the ptr to the deleted item in it, this is safe because
 * the item has not been wiped from segment */
bool
hashtable_delete(const char *key, const uint32_t klen,
        const bool try_del);

/*
 * delete the hashtable entry belong this specific item and all items
 * that are older than this entry
 */
bool
hashtable_evict(const char *oit_key, const uint32_t oit_klen,
                    uint64_t seg_id, uint64_t offset);

struct item *
hashtable_get(const char *key, const uint32_t klen, uint32_t *seg_id, uint64_t *cas);

void
hashtable_stat(int *n_item, int *n_extra_array);

