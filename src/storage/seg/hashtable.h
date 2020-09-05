#pragma once

#include "item.h"

/**
 * we use a bulk-chaining hash table,
 * each bucket is an array storing HASH_BUCKET_SIZE (fake) item pointers,
 * default HASH_BUCKET_SIZE = 8, which means one array stores 8 pointers
 * (except the first array which stores cas/lock/stat and 7 item pointers),
 * if the number of items hash to one bucket exceeds limit, a new array
 * is dynamically allocated and linked to this bucket, see below for details
 *
 *
 *
 *           hash table
 *       ┌─┬─┬─┬─┬─┬─┬─┬─┐      ┌───────────────┐
 *       │ │ │ │ │ │ │ │ ├─────▶│  extra bucket │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤      └───────────────┘
 *       │ │ │ │ │ │ │ │ │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤
 *       │ │ │ │ │ │ │ │ │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤      ┌───────────────┐
 *       │ │ │ │ │ │ │ │ ├─────▶│  extra bucket │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤      └───────────────┘
 *       │ │ │ │ │ │ │ │ │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤
 *       │ │ │ │ │ │ │ │ │
 *       ├─┼─┼─┼─┼─┼─┼─┼─┤
 *       │ │ │ │ │ │ │ │ │
 *       └─┴─┴─┴─┴─┴─┴─┴─┘
 *      each bucket is 64-byte
 *       bucket_info + 7 item
 *
 * As mentioned above, the first array of each bucket can only store 7 fake
 * item pointers because the first slot is used to store
 * lock (8-bit) + extra array cnt (8-bit) +  cas (32-bit)
 * we call this slot stat slot
 *
 * If the number of extra array is larger than zero,
 * then last slot is used to store pointer to the extra array.
 * For example, if we have 8 item pointers, then we will use 2 arrays
 * (one is part of the hash table and one extra array),
 * the first array stores stat + 6 item pointers + 1 pointer to next array
 * the second array stores 2 pointers
 *
 * Each fake item pointer is composed of
 * tag (16-bit) + seg_id (28-bit) + offset in the unit of 8-byte (20-bit)
 *
 *
 *              64-byte array (7 item "pointers" + one stat)
 *
 *   ┌───────┬───────┬───────┬───────┬───────┬───────┬───────┬───────┐
 *   │ stat  │ item  │ item  │ item  │ item  │ item  │ item  │ item  │
 *   │       │ info  │ info  │ info  │ info  │ info  │ info  │ info  │
 *   └───┬───┴───────┴───────┴───────┴───────┴───────┴───┬───┴───────┘
 *       └──────────┐                                ┌───┘
 *                  ▼                                ▼
 *      ┌──────────────────────┐         ┌──────────────────────┐
 *      │      32-bit cas      │         │      12-bit tag      │
 *      │ 8-bit # extra arrays │         │  8-bit freq counter  │
 *      │      8-bit lock      │         │    24-bit seg id     │
 *      │    16-bit unused     │         │    20-bit offset     │
 *      └──────────────────────┘         └──────────────────────┘
 *
 * Here is some data backing up the choice of parameters
 *
 * impact of load factor on the number of arrays in each bucket (array size 7)
 *
 * +--------------+--------+--------+--------+--------+--------+--------+
 * | load/#arrays |   1    |   2    |   3    |   4    |   5    |   6+   |
 * +--------------+--------+--------+--------+--------+--------+--------+
 * |       0.1831 | 99.99% | 0      | 0      |        |        |        |
 * |       0.2569 | 99.94% | 0.06%  | 0      |        |        |        |
 * |       0.3661 | 99.51% | 0.49%  | 0      |        |        |        |
 * |       0.5138 | 96.93% | 3.07%  | 0      |        |        |        |
 * |       0.7323 | 95.32% | 14.65% | 0.03%  |        |        |        |
 * |       1.0275 | 56.96% | 42.30% | 0.73%  | 0      |        |        |
 * |       1.4646 | 19.85% | 70.39% | 9.66%  | 0.10%  | 0      |        |
 * |       2.0551 | 2.56%  | 50.42% | 43.31% | 3.67%  | 0.04%  |        |
 * |       4.1101 | 0      | 0.18%  | 8.01%  | 41.08% | 39.97% | 10.76% |
 * +--------------+--------+--------+--------+--------+--------+--------+
 *
 *
 * NOTE: the nubmer of arrays depend on the array size, the table above is
 * calculated using array size 8, if we use larger array, then the corresponding
 * number will be smaller
 *
 *
 *
 * Hash collision rate vs tag length (#bits) and hash table load
 * +-----------+--------+--------+--------+--------+--------+--------+--------+
 * | load/bits |   4    |   6    |   8    |   10   |   12   |   14   |   16   |
 * +-----------+--------+--------+--------+--------+--------+--------+--------+
 * |    0.1284 | 0.0240 | 0.0062 | 0.0016 | 0.0004 | 0.0001 | 0.0000 | 0.0000 |
 * |    0.2574 | 0.0902 | 0.0245 | 0.0063 | 0.0016 | 0.0004 | 0.0001 | 0.0000 |
 * |    0.5296 | 0.3146 | 0.0976 | 0.0259 | 0.0065 | 0.0017 | 0.0004 | 0.0001 |
 * |    1.1329 | 0.8137 | 0.3636 | 0.1086 | 0.0286 | 0.0073 | 0.0018 | 0.0005 |
 * |    2.1837 | 0.9960 | 0.8059 | 0.3546 | 0.1055 | 0.0278 | 0.0069 | 0.0017 |
 * |    4.2468 | 1.0000 | 0.9964 | 0.8027 | 0.3449 | 0.1018 | 0.0265 | 0.0069 |
 * +-----------+--------+--------+--------+--------+--------+--------+--------+
 *
 *
 */


struct hash_table {
    uint32_t hash_power;
    uint64_t hash_mask; /* avoid repeated computation*/
    uint64_t *table;
};


void
hashtable_setup(uint32_t hash_power);
void
hashtable_teardown(void);


bool
hashtable_del_and_put(struct item *it);

void
hashtable_put(struct item *it, const uint64_t seg_id, const uint64_t offset);

/* delete the item from hashtable */
bool
hashtable_delete(const char *key, const uint32_t klen);

bool
hashtable_delete_it(const char *oit_key, const uint32_t oit_klen,
                    const uint64_t seg_id, const uint64_t offset);
/*
 * delete the hashtable entry belong to this specific item and all items
 * that are older than this entry
 */
bool
hashtable_evict(const char *oit_key, const uint32_t oit_klen, uint64_t seg_id,
        uint64_t offset);

struct item *
hashtable_get(const char *key, const uint32_t klen, int32_t *seg_id,
        uint64_t *cas);


bool
hashtable_relink_it(const char *oit_key, const uint32_t oit_klen,
        const uint64_t old_seg_id, const uint64_t old_offset,
        const uint64_t new_seg_id, const uint64_t new_offset);

/**
 * debugging functions
 * check whether an item (specified using seg + offset) is
 * in the hashtable
 */
bool
hashtable_check_it(const char *oit_key, const uint32_t oit_klen,
        const uint64_t seg_id, const uint64_t offset);

/**
 * debugging functions
 * get hash table stat
 */
void
hashtable_stat(int *n_item, int *n_extra_array);

/**
 * debugging functions
 * find whether there are any items from a certain segment
 * is in the hash table
 */
void
scan_hashtable_find_seg(int32_t target_seg_id);


int hashtable_get_it_freq(const char *oit_key, const uint32_t oit_klen,
                          const uint64_t old_seg_id, const uint64_t old_offset);

struct item *
hashtable_get_no_incr(const char *key, const uint32_t klen, int32_t *seg_id,
                      uint64_t *cas);