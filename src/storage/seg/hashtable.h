#pragma once

#include "item.h"

/**
 * bulk-chaining hashtable
 * we use a bulk-chaining hash table, hash table is divided into buckets, where
 * each bucket is an array has HASH_BUCKET_SIZE slots to store information.
 * default HASH_BUCKET_SIZE = 8, which means one bucket (array) has 8 slots.
 * the first/head bucket stores bucket info (cas/lock/stat) and 7 item info.
 *
 * Bucket overflow
 * if there are more than 7 items hashed into this bucket,
 * we call the bucket overflows, a new bucket is allocated (currently malloc),
 * the last slot of the head bucket becomes a pointer to the allocated bucket,
 * the item info stored in the last slot of the head bucket is copied to the
 * first slot of the new bucket.
 * For example, if we have 8 item pointers, then we will use 2 buckets
 * one is part of the hash table and one is allocated on-demand,
 * the first bucket stores bucket info + 6 item info + 1 pointer to next bucket
 * the second array stores 2 item info (no bucket info stored)

 * On an hash table load (~0.75), there are around 15% (or 5%) buckets
 * overflow, meaning most item info are stored within the hash table
 *
 * see below for details
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
 *   overflown buckets do not have bucket_info
 *
 * Bucket info
 * As mentioned above, the first array of head bucket only store 7 item info,
 * because the first slot store bucket info:
 * lock (8-bit) + bucket chain length (8-bit) + cas (32-bit) +
 * shared last access timestamp
 *
 * If the number of extra array is larger than zero,
 * then last slot is used to store pointer to the extra array.
 *
 * Each item info (fake pointer) is composed of
 * tag (12-bit) + 8-bit frequency counter + seg_id (24-bit) +
 * offset in the unit of 8-byte (20-bit)
 *
 *
 *              64-byte bucket (7 item into + one stat)
 *
 *   ┌───────┬───────┬───────┬───────┬───────┬───────┬───────┬───────┐
 *   │ stat  │ item  │ item  │ item  │ item  │ item  │ item  │ item  │
 *   │       │ info  │ info  │ info  │ info  │ info  │ info  │ info  │
 *   └───┬───┴───────┴───────┴───────┴───────┴───────┴───┬───┴───────┘
 *       └──────────┐                                ┌───┘
 *                  ▼                                ▼
 *      ┌────────────────────────┐         ┌──────────────────────┐
 *      │      32-bit cas        │         │      12-bit tag      │
 *      │ 8-bit bucket chain len │         │  8-bit freq counter  │
 *      │      8-bit lock        │         │    24-bit seg id     │
 *      │    16-bit unused       │         │    20-bit offset     │
 *      └────────────────────────┘         └──────────────────────┘
 *
 *
 * Here are some data backing up the choice of parameters
 *
 * impact of load factor on the length of bucket chain
 *
 * +--------------+--------+--------+--------+--------+--------+--------+
 * | load / bucket|   1    |   2    |   3    |   4    |   5    |   6+   |
 * | chain length |        |        |        |        |        |        |
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
 * NOTE: the bucket chain length depends on the bucket size, the table above is
 * calculated using bucket size 8 - each bucket stores 8 items
 * if we use larger bucket, then the corresponding number will be smaller
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


/* todo(juncheng):
 * 1. replace overflow bucket memory allocation from malloc to static allocation,
 *      this is not necessary at this moment, a proper allocated hash table
 *      should not have too many overflown buckets, maybe add a stat
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


void
hashtable_put(struct item *it, uint64_t seg_id, uint64_t offset);

/* delete the item from hashtable */
bool
hashtable_delete(const struct bstring *key);

bool
hashtable_delete_it(struct item *it, uint64_t seg_id, uint64_t offset);

/*
 * delete the hashtable entry of to this item and all entries
 * of older versions of this item
 */
bool
hashtable_evict(const char *oit_key, uint32_t oit_klen, uint64_t seg_id,
        uint64_t offset);

struct item *
hashtable_get(const char *key, uint32_t klen, int32_t *seg_id,
        uint64_t *cas);


bool
hashtable_relink_it(const char *oit_key, uint32_t oit_klen,
        uint64_t old_seg_id, uint64_t old_offset,
        uint64_t new_seg_id, uint64_t new_offset);

/**
 * debugging functions
 * check whether an item (specified using seg + offset) is
 * in the hashtable
 */
bool
hashtable_check_it(const char *oit_key, uint32_t oit_klen,
        uint64_t seg_id, uint64_t offset);


/**
 * internal functions
 * get hash table stat, the number of used slots and the number of used buckets
 *
 */
void
hashtable_stat(int *item_cnt_ptr, int *bucket_cnt_ptr);


/**
 * debugging functions
 * find whether there are any items from a certain segment
 * is in the hash table
 */
void
scan_hashtable_find_seg(int32_t target_seg_id);


int hashtable_get_it_freq(const char *oit_key, uint32_t oit_klen,
                          uint64_t old_seg_id, uint64_t old_offset);

struct item *
hashtable_get_no_freq_incr(const char *key, uint32_t klen, int32_t *seg_id,
                           uint64_t *cas);