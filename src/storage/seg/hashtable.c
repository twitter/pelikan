
#define XXH_INLINE_ALL

#include "hashtable.h"
#include "hash/xxhash.h"
#include "item.h"
#include "seg.h"

#include <cc_mm.h>
#include <hash/cc_murmur3.h>

#include <stdlib.h>
#include <sys/mman.h>
#include <sysexits.h>
#include <x86intrin.h>

#define CACHE_ALIGN_SIZE 64

/* need to be multiple of 8 to make use of 64-byte cache line */
#define BUCKET_SIZE 8u
#define BUCKET_SIZE_BITS 3u


#define TAG_MASK 0xffff000000000000ul
#define SEG_ID_MASK 0x0000fffffff00000ul
#define OFFSET_MASK 0x00000000000ffffful

#define LOCK_MASK 0xff00000000000000ul
#define ARRAY_CNT_MASK 0x00ff000000000000ul
#define CAS_MASK 0x00000000fffffffful

#define LOCKED 0x0100000000000000ul
#define UNLOCKED 0x0000000000000000ul


extern seg_metrics_st *seg_metrics;
extern bool use_cas;

static struct hash_table hash_table;
static bool hash_table_initialized = false;

static uint32_t murmur3_iv = 0x3ac5d673;


#define HASHSIZE(_n) (1ULL << (_n))
#define HASHMASK(_n) (HASHSIZE(_n) - 1)

#define GET_HV(key, klen) _get_hv_xxhash(key, klen)
/* tag has to start from 1 to avoid differentiate item_info from pointer */
#define GET_TAG(v) (((v)&TAG_MASK))
#define CAL_TAG_FROM_HV(hv) (((hv)&TAG_MASK) | 0x0001000000000000ul)
#define GET_BUCKET(hv) (&hash_table.table[(hv)&hash_table.hash_mask])

/* TODO(jason): change to atomic */
#define GET_CAS(bucket_ptr) (*bucket_ptr) & CAS_MASK
#define GET_ARRAY_CNT(bucket_ptr) (((*bucket_ptr) & ARRAY_CNT_MASK) >> 48u)
#define INCR_ARRAY_CNT(bucket_ptr) ((*(bucket_ptr)) += 0x0001000000000000ul)

#define CAS_SLOT(slot, expect_ptr, new)                                        \
    __atomic_compare_exchange_n(                                               \
            slot, expect_ptr, new, false, __ATOMIC_RELEASE, __ATOMIC_RELAXED)


/* TODO(jason): it is better to use id for extra_array instead of pointer
 * otherwise we have to assume each pointer is 64-byte
 * add bucket array shrink
 * */

/* we assume little-endian here */
#define lock(bucket_ptr)                                                       \
    do {                                                                       \
        uint8_t locked = 0;                                                    \
        while (!CAS_SLOT(((uint8_t *)bucket_ptr + 7), &locked, 1)) {           \
            ASSERT(locked == 1);                                               \
            locked = 0;                                                        \
            usleep(1);                                                         \
        }                                                                      \
    } while (0)

#define unlock(bucket_ptr)                                                     \
    do {                                                                       \
        ASSERT(((*bucket_ptr) & LOCK_MASK) == LOCKED);                         \
        *bucket_ptr ^= LOCKED;                                                 \
    } while (0)

#define unlock_and_update_cas(bucket_ptr)                                      \
    do {                                                                       \
        ASSERT(((*bucket_ptr) & LOCK_MASK) != 0);                              \
        *bucket_ptr = (*bucket_ptr + 1) ^ LOCKED;                              \
    } while (0)

#ifdef use_atomic_set
#    undef lock
#    undef unlock
#    undef unlock_and_update_cas
#    define lock(bucket_ptr)                                                   \
        do {                                                                   \
            while (__atomic_test_and_set(((uint8_t *)bucket_ptr + 7)),         \
                    __ATOMIC_ACQUIRE) {                                        \
                usleep(1);                                                     \
            }                                                                  \
        } while (0)

#    define unlock(bucket_ptr)                                                 \
        do {                                                                   \
            __atomic_clear((((uint8_t *)bucket_ptr) + 7), __ATOMIC_RELEASE);   \
        } while (0)

#    define unlock_and_update_cas(bucket_ptr)                                  \
        do {                                                                   \
            *bucket_ptr += 1;                                                  \
        __atomic_clear(((uint8_t *)bucket_ptr) + 7), __ATOMIC_RELEASE);        \
        } while (0)
#endif

#ifdef no_lock
#    undef lock
#    undef unlock
#    undef unlock_and_update_cas
#    define lock(bucket_ptr)
#    define unlock(bucket_ptr)
#    define unlock_and_update_cas(bucket_ptr) ((*(bucket_ptr)) += 1)
#endif


/**
 * this is placed here because it is called within bucket lock and it
 * needs to parse item_info
 *
 */

static inline struct item *
_info_to_item(uint64_t item_info)
{
    uint64_t seg_id = ((item_info & SEG_ID_MASK) >> 20u);
    uint64_t offset = (item_info & OFFSET_MASK) << 3u;
    ASSERT(seg_id < heap.max_nseg);
    ASSERT(offset < heap.seg_size);
    return (struct item *)(heap.base + heap.seg_size * seg_id + offset);
}

static inline void
_item_free(uint64_t item_info)
{
    uint64_t seg_id = ((item_info & SEG_ID_MASK) >> 20u);
    uint64_t offset = (item_info & OFFSET_MASK) << 3u;
    uint32_t sz = item_ntotal(
            (struct item *)(heap.base + heap.seg_size * seg_id + offset));

    __atomic_fetch_sub(&heap.segs[seg_id].occupied_size, sz, __ATOMIC_RELAXED);
    __atomic_fetch_sub(&heap.segs[seg_id].n_item, 1, __ATOMIC_RELAXED);
}

static inline bool
_same_item(const char *key, uint32_t klen, uint64_t item_info)
{
    struct item *oit = _info_to_item(item_info);
    return ((oit->klen == klen) && cc_memcmp(item_key(oit), key, klen) == 0);
}


/*
 * Allocate table given size
 */
static inline uint64_t *
_hashtable_alloc(uint64_t n_slot)
{
    uint64_t *table =
            aligned_alloc(CACHE_ALIGN_SIZE, sizeof(uint64_t) * n_slot);
    if (table == NULL) {
        log_crit("cannot create hash table");
        exit(EX_CONFIG);
    }
    cc_memset(table, 0, sizeof(uint64_t) * n_slot);

#ifdef MADV_HUGEPAGE
    /* USE_HUGEPAGE */
    madvise(table, sizeof(uint64_t) * n_slot, MADV_HUGEPAGE);
#endif

    return table;
}

void
hashtable_setup(uint32_t hash_power)
{
    uint64_t n_slot;

    ASSERT(hash_power > 0);

    if (hash_table_initialized) {
        log_warn("hash table has been initialized");
        hashtable_teardown();
    }

    /* init members */
    hash_table.hash_power = hash_power;
    n_slot = HASHSIZE(hash_power);
    hash_table.hash_mask =
            (n_slot - 1) & (0xffffffffffffffff << BUCKET_SIZE_BITS);

    /* alloc table */
    hash_table.table = _hashtable_alloc(n_slot);

    hash_table_initialized = true;

    log_info("create hash table of %" PRIu64 " elements %" PRIu64 " buckets",
            n_slot, n_slot >> BUCKET_SIZE_BITS);
}

void
hashtable_teardown(void)
{
    if (!hash_table_initialized) {
        log_warn("hash table is not initialized");
        return;
    }

    cc_free(hash_table.table);
    hash_table.table = NULL;

    hash_table_initialized = false;
}

static inline uint64_t
_get_hv_murmur3(const char *key, size_t klen)
{
    uint64_t hv[2];

    hash_murmur3_128_x64(key, klen, murmur3_iv, hv);

    return hv[0];
}

static inline uint64_t
_get_hv_xxhash(const char *key, size_t klen)
{
    return XXH3_64bits(key, klen);

    /* maybe this API is preferred
     *   uint64_t hv = XXH3_64bits_dispatch(key, klen);
     */
}


static inline void
_insert_item_in_bucket_array(uint64_t *array, int n_array_item, struct item *it,
        uint64_t tag, uint64_t insert_item_info, bool *inserted, bool *deleted)
{
    uint64_t item_info;

    for (int i = 0; i < n_array_item; i++) {
        item_info = __atomic_load_n(&array[i], __ATOMIC_ACQUIRE);

        if (GET_TAG(item_info) != tag) {
            if (!*inserted && item_info == 0) {
                *inserted = CAS_SLOT(array + i, &item_info, insert_item_info);
                if (*inserted) {
                    /* we have inserted, so when we encounter old entry,
                     * just reset the slot */
                    insert_item_info = 0;
                }
            }
            continue;
        }
        /* a potential hit */
        if (!_same_item(item_key(it), it->klen, item_info)) {
            continue;
        }
        /* we have found the item, now atomic update */
        *deleted = CAS_SLOT(array + i, &item_info, insert_item_info);
        if (*deleted) {
            /* update successfully */
            *inserted = true;
            return;
        }

        /* the slot has changed, double-check this updated item */
        if (item_info == 0) {
            /* the item is evicted */
            *inserted = CAS_SLOT(array + i, &item_info, insert_item_info);
            /* whether it succeeds or fails, we return,
             * see below for why we return when it fails, as an alternative
             * we can re-start the put here */
            return;
        }

        if (!_same_item(item_key(it), it->klen, item_info)) {
            /* original item evicted, a new item is inserted - rare */
            continue;
        }
        /* the slot has been updated with the same key, replace it */
        *deleted = CAS_SLOT(array + i, &item_info, insert_item_info);
        if (*deleted) {
            /* update successfully */
            *inserted = true;
            return;
        } else {
            /* AGAIN? this should be very rare, let's give up
             * the possible consequences of giving up:
             * 1. current item might not be inserted, not a big deal
             * because at such high concurrency, we cannot tell whether
             * current item is the most updated one or the the one in the
             * slot
             * 2. current item is inserted in an early slot, then we
             * will have two entries for the same key, this if fine as
             * well, because at eviction time, we will remove them
             **/
            return;
        }
    }
}

/**
 * because other threads can update the bucket at the same time,
 * so we always have to check existence before put
 *
 *
 */
/**
 * insert logic
 * insert has two steps, insert and delete (success or not found)
 * insert and delete must be completed in the same pass of scanning,
 * otherwise it cannot guarantee correctness
 *
 *
 * scan through all slots,
 * 1. if we found the item, cas
 * 1-1. if cas succeeds, return
 * 1-2. if cas fails, compare item
 *          if same, free the item, return, otherwise, continue
 * 2. if we found an available slot, cas
 * 2-1. if cas succeeds, continue scanning and delete
 * 2-2. if cas fails, continue
 */

void
hashtable_put(struct item *it, const uint64_t seg_id, const uint64_t offset)
{
    const char *key = item_key(it);
    const uint32_t klen = item_nkey(it);

    uint64_t hv = GET_HV(key, klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);
    uint64_t *bucket = GET_BUCKET(hv);

    uint64_t *array = bucket;
    INCR(seg_metrics, hash_insert);

    /* 16-bit tag, 28-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info, insert_item_info = tag;
    insert_item_info |= (seg_id << 20u) | (offset >> 3u);
    log_vverb("hashtable insert %" PRIu64, insert_item_info);

    ASSERT(((insert_item_info & SEG_ID_MASK) >> 20u) == seg_id);
    ASSERT(((insert_item_info & OFFSET_MASK) << 3u) == offset);

    lock(bucket);

    int extra_array_cnt = GET_ARRAY_CNT(bucket);
    int array_size;
    do {
        /* this loop will be executed at least once */
        array_size = extra_array_cnt > 0 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

        for (int i = 0; i < array_size; i++) {
            if (array == bucket && i == 0) {
                continue;
            }

            item_info = array[i];
            if (GET_TAG(item_info) != tag) {
                if (insert_item_info != 0 && item_info == 0) {
                    array[i] = insert_item_info;
                    insert_item_info = 0;
                }
                continue;
            }
            /* a potential hit */
            if (!_same_item(key, klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            /* found the item, now atomic update (or delete if already inserted)
             * x86 read and write 8-byte is always atomic */
            array[i] = insert_item_info;
            insert_item_info = 0;

            _item_free(item_info);

            goto finish;
        }

        if (insert_item_info == 0) {
            /* item has been inserted, do not check next array */
            goto finish;
        }

        extra_array_cnt -= 1;
        if (extra_array_cnt >= 0) {
            array = (uint64_t *)(array[BUCKET_SIZE - 1]);
        }
    } while (extra_array_cnt >= 0);

    /* we have searched every array, but have not found the old item
     * nor inserted new item - so we need to allocate a new array,
     * this is very rare */
    INCR(seg_metrics, hash_array_alloc);

    uint64_t *new_array = cc_zalloc(8 * BUCKET_SIZE);
    new_array[0] = array[BUCKET_SIZE - 1];
    new_array[1] = insert_item_info;
    insert_item_info = 0;

    __atomic_thread_fence(__ATOMIC_RELEASE);

    array[BUCKET_SIZE - 1] = (uint64_t)new_array;

    INCR_ARRAY_CNT(bucket);

finish:
    ASSERT(insert_item_info == 0);
    unlock_and_update_cas(bucket);
}


bool
hashtable_delete(const char *key, const uint32_t klen, const bool try_del)
{
    INCR(seg_metrics, hash_remove);

    bool deleted = false;

    uint64_t hv = GET_HV(key, klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);
    uint64_t *bucket = GET_BUCKET(hv);
    uint64_t *array = bucket;

    /* 16-bit tag, 28-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info;

    /* 8-bit lock, 8-bit bucket array cnt, 16-bit unused, 32-bit cas */

    lock(bucket);

    int extra_array_cnt = GET_ARRAY_CNT(bucket);
    int array_size;
    do {
        array_size = extra_array_cnt > 0 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

        for (int i = 0; i < array_size; i++) {
            if (array == bucket && i == 0) {
                continue;
            }

            item_info = array[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(key, klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            /* found the item, now delete */
            array[i] = 0;
            deleted = true;
            _item_free(item_info);
        }
        extra_array_cnt -= 1;
        array = (uint64_t *)(array[BUCKET_SIZE - 1]);
    } while (extra_array_cnt >= 0);


    unlock(bucket);
    return deleted;
}

/*
 * delete the hashtable entry only if item is the up-to-date/valid item
 *
 * TODO(jason): use version instead of locking might be better
 */
bool
hashtable_evict(const char *oit_key, const uint32_t oit_klen,
        const uint64_t seg_id, const uint64_t offset)
{
    INCR(seg_metrics, hash_remove);

    bool deleted = false;

    uint64_t hv = GET_HV(oit_key, oit_klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);
    uint64_t *bucket = GET_BUCKET(hv);
    uint64_t *array = bucket;

    /* 16-bit tag, 28-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info;
    uint64_t oit_info = tag | (seg_id << 20u) | (offset >> 3u);

    /* we only want to delete entries of the object as old as oit,
     * so we need to find oit first, once we find it, we will delete
     * all entries of this key */
    bool delete_rest = false;

    lock(bucket);

    int extra_array_cnt = GET_ARRAY_CNT(bucket);
    int array_size;
    do {
        array_size = extra_array_cnt > 0 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

        for (int i = 0; i < array_size; i++) {
            if (array == bucket && i == 0) {
                continue;
            }

            item_info = array[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(oit_key, oit_klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }

            if (item_info == oit_info) {
                _item_free(item_info);
                deleted = true;
                delete_rest = true;
                array[i] = 0;
            } else {
                if (delete_rest) {
                    _item_free(item_info);
                    array[i] = 0;
                } else {
                    /* this is the newest entry */
                    delete_rest = true;
                }
            }
        }
        extra_array_cnt -= 1;
        array = (uint64_t *)(array[BUCKET_SIZE - 1]);
    } while (extra_array_cnt >= 0);

    unlock(bucket);

    return deleted;
}

struct item *
hashtable_get(
        const char *key, const uint32_t klen, int32_t *seg_id, uint64_t *cas)
{
    uint64_t hv = GET_HV(key, klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);
    uint64_t *bucket = GET_BUCKET(hv);
    uint64_t *array = bucket;
    uint64_t offset;

    /* 16-bit tag, 28-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info;

    int extra_array_cnt = GET_ARRAY_CNT(bucket);
    int array_size;
    do {
        array_size = extra_array_cnt > 0 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

        for (int i = 0; i < array_size; i++) {
            if (array == bucket && i == 0) {
                continue;
            }

            item_info = array[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(key, klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            if (cas) {
                *cas = GET_CAS(bucket);
            }

            *seg_id = (item_info & SEG_ID_MASK) >> 20u;
            offset = (item_info & OFFSET_MASK) << 3u;
            return (struct item *)(heap.base + heap.seg_size * (*seg_id) +
                    offset);
        }
        extra_array_cnt -= 1;
        array = (uint64_t *)(array[BUCKET_SIZE - 1]);
    } while (extra_array_cnt >= 0);


    return NULL;
}

bool
hashtable_check_it(const char *oit_key, const uint32_t oit_klen,
        const uint64_t seg_id, const uint64_t offset)
{
    INCR(seg_metrics, hash_remove);

    uint64_t hv = GET_HV(oit_key, oit_klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);
    uint64_t *bucket = GET_BUCKET(hv);
    uint64_t *array = bucket;

    uint64_t oit_info = tag | (seg_id << 20u) | (offset >> 3u);


    lock(bucket);

    int extra_array_cnt = GET_ARRAY_CNT(bucket);
    int array_size;
    do {
        array_size = extra_array_cnt > 0 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

        for (int i = 0; i < array_size; i++) {
            if (oit_info == array[i]) {
                unlock(bucket);
                return true;
            }
        }
        extra_array_cnt -= 1;
        array = (uint64_t *)(array[BUCKET_SIZE - 1]);
    } while (extra_array_cnt >= 0);

    unlock(bucket);

    return false;
}

void
hashtable_stat(int *item_cnt_ptr, int *extra_array_cnt_ptr)
{
#define BUCKET_HEAD(idx) (&hash_table.table[(idx)*BUCKET_SIZE])

    int item_cnt = 0;
    int extra_array_cnt_sum = 0, extra_array_cnt;
    int duplicate_item_cnt = 0;
    uint64_t item_info;
    uint64_t *bucket, *array;
    int array_size;
    int n_bucket = HASHSIZE(hash_table.hash_power - BUCKET_SIZE_BITS);

    for (uint64_t bucket_idx = 0; bucket_idx < n_bucket; bucket_idx++) {
        bucket = BUCKET_HEAD(bucket_idx);
        array = bucket;
        extra_array_cnt = GET_ARRAY_CNT(bucket);
        extra_array_cnt_sum += extra_array_cnt;
        do {
            array_size = extra_array_cnt >= 1 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

            for (int i = 0; i < array_size; i++) {
                if (array == bucket && i == 0) {
                    continue;
                }

                item_info = array[i];
                if (item_info != 0) {
                    item_cnt += 1;
                }
            }
            extra_array_cnt -= 1;
            array = (uint64_t *)(array[BUCKET_SIZE - 1]);
        } while (extra_array_cnt >= 0);
    }

    if (item_cnt_ptr != NULL) {
        *item_cnt_ptr = item_cnt;
    }
    if (extra_array_cnt_ptr != NULL) {
        *extra_array_cnt_ptr = extra_array_cnt_sum;
    }

    log_info("hashtable %d entries, %d extra_arrays", item_cnt,
            extra_array_cnt_sum);

#undef BUCKET_HEAD
}

void
scan_hashtable_find_seg(int32_t target_seg_id)
{
#define BUCKET_HEAD(idx) (&hash_table.table[(idx)*BUCKET_SIZE])
    int extra_array_cnt;
    uint64_t item_info;
    uint64_t *bucket, *array;
    int array_size;
    uint64_t seg_id;
    uint64_t offset;
    struct item *it;
    int n_bucket = HASHSIZE(hash_table.hash_power - BUCKET_SIZE_BITS);

    for (uint64_t bucket_idx = 0; bucket_idx < n_bucket; bucket_idx++) {
        bucket = BUCKET_HEAD(bucket_idx);
        array = bucket;
        extra_array_cnt = GET_ARRAY_CNT(bucket);
        int extra_array_cnt0 = extra_array_cnt;
        do {
            array_size = extra_array_cnt >= 1 ? BUCKET_SIZE - 1 : BUCKET_SIZE;

            for (int i = 0; i < array_size; i++) {
                if (array == bucket && i == 0) {
                    continue;
                }

                item_info = array[i];

                if (item_info == 0) {
                    continue;
                }

                seg_id = ((item_info & SEG_ID_MASK) >> 20u);
                if (target_seg_id == seg_id) {
                    offset = (item_info & OFFSET_MASK) << 3u;
                    it = (struct item *)(heap.base + heap.seg_size * seg_id +
                            offset);
                    log_warn("find item (%.*s) len %d on seg %d offset %d, item_info "
                             "%lu, i %d, extra %d %d",
                            it->klen, item_key(it), it->klen, seg_id, offset, item_info, i,
                            extra_array_cnt0, extra_array_cnt);
                }
            }
            extra_array_cnt -= 1;
            array = (uint64_t *)(array[BUCKET_SIZE - 1]);
        } while (extra_array_cnt >= 0);
    }

#undef BUCKET_HEAD
}