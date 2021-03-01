
#define XXH_INLINE_ALL

#include "hashtable.h"
#include "xxhash.h"
#include "item.h"
#include "seg.h"

#include <cc_mm.h>
#include <hash/cc_murmur3.h>

#include <stdlib.h>
#include <sys/mman.h>
#include <sysexits.h>
#include <x86intrin.h>

/* TODO(jason): use static allocated array
 * TODO(jason): add bucket array shrink
 * */


/* the size of bucket in bytes, used in malloc alignment */
#define N_BYTE_PER_BUCKET                    64

/* the number of slots in one bucket */
#define N_SLOT_PER_BUCKET                   8u

/* the number of slots expressed in number of bits  8 == 2^3 */
#define N_SLOT_PER_BUCKET_LOG2           3u

/* mask for item_info */
#define TAG_MASK                0xfff0000000000000ul
#define FREQ_MASK               0x000ff00000000000ul
#define SEG_ID_MASK             0x00000ffffff00000ul
#define OFFSET_MASK             0x00000000000ffffful

#define TAG_BIT_SHIFT           52ul
#define FREQ_BIT_SHIFT          44ul
#define SEG_ID_BIT_SHIFT        20ul
#define OFFSET_UNIT_IN_BIT      3ul     /* offset is in 8-byte unit */

/* this bit indicates whether the frequency has increased in the current sec */
#define FREQ_INC_INDICATOR_MASK  0x0008000000000000ul
#define CLEAR_FREQ_SMOOTH_MASK   0xfff7fffffffffffful

/* mast for bucket info */
#define LOCK_MASK               0xff00000000000000ul
#define BUCKET_CHAIN_LEN_MASK   0x00ff000000000000ul
#define TS_MASK                 0x0000ffff00000000ul  /* ts in bucket info */
#define CAS_MASK                0x00000000fffffffful

#define LOCK_BIT_SHIFT              56ul
#define BUCKET_CHAIN_LEN_BIT_SHIFT  48ul
#define TS_BIT_SHIFT                32ul


/* ts from proc ts, we only need 16-bit */
#define PROC_TS_MASK            0x000000000000fffful

/* a per-bucket spin lock */
#define LOCKED                  0x0100000000000000ul
#define UNLOCKED                0x0000000000000000ul

extern seg_metrics_st *seg_metrics;

static struct hash_table    hash_table;
static bool                 hash_table_initialized = false;
static __thread __uint128_t g_lehmer64_state       = 1;

#define HASHSIZE(_n)        (1ULL << (_n))
#define HASHMASK(_n)        (HASHSIZE(_n) - 1)
#define CAL_HV(key, klen)   _get_hv_xxhash(key, klen)

/* tags is calculated in two places,
 * one is extracted from hash value, the other is extracted from item info,
 * we use the top 12 bits of hash value as tag and
 * store it in the top 12 bits of item info,
 * note that we make tag to start from 1, so when we calculate the true tag
 * we perform OR with 0x0001000000000000ul */
#define GET_TAG(item_info)      ((item_info) & TAG_MASK)
#define GET_FREQ(item_info)     (((item_info) & FREQ_MASK) >> FREQ_BIT_SHIFT)
#define GET_SEG_ID(item_info)   (((item_info) & SEG_ID_MASK) >> SEG_ID_BIT_SHIFT)
#define GET_OFFSET(item_info)   (((item_info) & OFFSET_MASK) << OFFSET_UNIT_IN_BIT)
#define CLEAR_FREQ(item_info)   ((item_info) & (~FREQ_MASK))

#define CAL_TAG_FROM_HV(hv) (((hv) & TAG_MASK) | 0x0010000000000000ul)
#define GET_BUCKET(hv)      (&hash_table.table[((hv) & (hash_table.hash_mask))])

#define GET_TS(bucket_ptr)          (((*(bucket_ptr)) & TS_MASK) >> TS_BIT_SHIFT)
#define GET_CAS(bucket_ptr)         ((*(bucket_ptr)) & CAS_MASK)

/* calculate the number of buckets in the bucket chain */
#define GET_BUCKET_CHAIN_LEN(bucket_ptr)                                       \
    ((((*(bucket_ptr)) & BUCKET_CHAIN_LEN_MASK) >> BUCKET_CHAIN_LEN_BIT_SHIFT) + 1)
#define INCR_BUCKET_CHAIN_LEN(bucket_ptr)                                      \
                ((*(bucket_ptr)) += 0x0001000000000000ul)

#define CAS_SLOT(slot_ptr, expect_ptr, new_val)                                \
    __atomic_compare_exchange_n(                                               \
        (slot_ptr), (expect_ptr), (new_val), false,                            \
        __ATOMIC_RELEASE, __ATOMIC_RELAXED                                     \
    )

#if defined HASHTABLE_DBG
#define SET_BUCKET_MAGIC(bucket_ptr)                                           \
    (*(bucket_ptr)) = ((*(bucket_ptr)) | 0x0000ffff00000000ul)
#define CHECK_BUCKET_MAGIC(bucket_ptr)                                         \
    ASSERT(((*(bucket_ptr)) & 0x0000ffff00000000ul) == 0x0000ffff00000000ul)

#undef GET_BUCKET
static uint64_t* GET_BUCKET(uint64_t hv)
{
    uint64_t *bucket_ptr = (&hash_table.table[((hv) & hash_table.hash_mask)]);
    CHECK_BUCKET_MAGIC(bucket_ptr);
    return bucket_ptr;
}

#else
#define SET_BUCKET_MAGIC(bucket_ptr)
#define CHECK_BUCKET_MAGIC(bucket_ptr)
#endif

#define use_atomic_set
/* we assume little-endian here */
#define lock(bucket_ptr)                                                       \
    do {                                                                       \
        uint8_t locked = 0;                                                    \
        while (!CAS_SLOT(((uint8_t *)bucket_ptr + 7), &locked, 1)) {           \
            ASSERT(locked == 1);                                               \
            locked = 0;                                                        \
            ;                                                         \
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
#undef lock
#undef unlock
#undef unlock_and_update_cas
#define lock(bucket_ptr)                                                        \
    do {                                                                        \
        while (__atomic_test_and_set(                                           \
            ((uint8_t *)(bucket_ptr) + 7), __ATOMIC_ACQUIRE)) {                 \
            ;                                                                   \
        }                                                                       \
    } while (0)

#define unlock(bucket_ptr)                                                      \
    do {                                                                        \
        __atomic_clear(((uint8_t *)(bucket_ptr) + 7), __ATOMIC_RELEASE);        \
    } while (0)

#define unlock_and_update_cas(bucket_ptr)                                       \
    do {                                                                        \
        *bucket_ptr += 1;                                                       \
        __atomic_clear(((uint8_t *)(bucket_ptr) + 7), __ATOMIC_RELEASE);        \
    } while (0)
#endif

#ifdef no_lock
#undef lock
#undef unlock
#undef unlock_and_update_cas
#define lock(bucket_ptr)
#define unlock(bucket_ptr)
#define unlock_and_update_cas(bucket_ptr) ((*(bucket_ptr)) += 1)
#endif

/**
 * this is placed here because it is called within bucket lock and it
 * needs to parse item_info
 *
 */

static inline struct item *
_info_to_item(uint64_t item_info)
{
    uint64_t seg_id = GET_SEG_ID(item_info);
    uint64_t offset = GET_OFFSET(item_info);
    ASSERT(seg_id < heap.max_nseg);
    ASSERT(offset < heap.seg_size);

    return (struct item *) (heap.base + heap.seg_size * seg_id + offset);
}

static inline void
_item_free(uint64_t item_info, bool mark_tombstone)
{
    struct item *it;
    uint64_t    seg_id = GET_SEG_ID(item_info);
    uint64_t    offset = GET_OFFSET(item_info);
    it = (struct item *) (heap.base + heap.seg_size * seg_id + offset);
    uint32_t sz = item_ntotal(it);

    __atomic_fetch_sub(&heap.segs[seg_id].occupied_size, sz, __ATOMIC_RELAXED);
    __atomic_fetch_sub(&heap.segs[seg_id].n_item, 1, __ATOMIC_RELAXED);

    ASSERT(__atomic_load_n(&heap.segs[seg_id].n_item, __ATOMIC_RELAXED) >= 0);
    ASSERT(__atomic_load_n(&heap.segs[seg_id].occupied_size, __ATOMIC_RELAXED)
        >= 0);

    if (mark_tombstone) {
        it->deleted = true;
    }
}

static inline bool
_same_item(const char *key, uint32_t klen, uint64_t item_info)
{
    struct item *oit = _info_to_item(item_info);
    return ((oit->klen == klen) && cc_memcmp(item_key(oit), key, klen) == 0);
}

static inline uint64_t
_build_item_info(uint64_t tag, uint64_t seg_id, uint64_t offset)
{
    ASSERT(offset % 8 == 0);
    uint64_t item_info = tag | (seg_id << 20u) | (offset >> 3u);
    return item_info;
}

#define SET_BIT(u64, pos) ((u64) | (1ul << (pos)))
#define GET_BIT(u64, pos) ((u64) & (1ul << (pos)))
#define CHECK_BIT(u64, pos) GET_BIT(u64, pos)
#define CLEAR_BIT(u64, pos) ((u64) & (~(1ul << (pos))))

static inline uint64_t
prand(void)
{
    g_lehmer64_state *= 0xda942042e4dd58b5;
    return (uint64_t) g_lehmer64_state;
}

static inline uint64_t
_get_hv_xxhash(const char *key, size_t klen)
{
    return XXH3_64bits(key, klen);
}


/*
 * we use an approximate and probabilistic frequency counter
 * the freq counter has 8 bits, we set i-th bit if
 * 1. all bits before i has been set
 * 2. current time in sec mod (1 << i) == 0
 *
 * this avoids
 * 1. counting temporal bursts in seconds
 * 2. by counting only once per sec,
 *      the i-th bit is set with probability 1/(1<<i)
 *
 */
//#ifdef COMMENT
//static inline uint64_t
//_incr_freq(uint64_t item_info)
//{
//#define FREQ_BIT_START 44u
//    if (last_sec_update != time_proc_sec()) {
//        last_sec_update = time_proc_sec();
//        cur_sec_freq_bit = 0;
//        while (((last_sec_update >> (cur_sec_freq_bit)) & 1u) == 0 && cur_sec_freq_bit < 8) {
//            cur_sec_freq_bit += 1;
//        }
//        cur_sec_freq_bit += 1;
//        ASSERT(FREQ_BIT_START + cur_sec_freq_bit <= 63);
//    }
//
//
//    for (unsigned int i = 0; i < cur_sec_freq_bit; i++) {
//        if (GET_BIT(item_info, FREQ_BIT_START + i) == 0) {
//            /* current bit is not set */
//            item_info = SET_BIT(item_info, FREQ_BIT_START + i);
//            /* set one bit at a time */
//            break;
//        }
//    }
//
//    return item_info;
//}
//#endif

//static inline uint64_t
//_incr_freq(uint64_t item_info)
//{
//#define FREQ_BIT_START 44u
//    uint64_t freq = GET_FREQ(item_info);
//    if (freq == 0 || prand() % freq == 0) {
//        freq = freq < 255 ? freq + 1 : 255;
//    }
//
//    item_info = (CLEAR_FREQ(item_info)) | (freq << FREQ_BIT_START);
//    return item_info;
//}

/*
 * Allocate table given size
 */
static inline uint64_t *
_hashtable_alloc(uint64_t n_slot)
{
    uint64_t *table =
                 aligned_alloc(N_BYTE_PER_BUCKET, sizeof(uint64_t) * n_slot);
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

    ASSERT(hash_power > 0);

    if (hash_table_initialized) {
        log_warn("hash table has been initialized");
        hashtable_teardown();
    }

    /* init members */
    hash_table.hash_power = hash_power;
    uint64_t n_slot = HASHSIZE(hash_power);
    /* N_SLOT_PER_BUCKET slots are in one bucket, so hash_mask last
     * N_SLOT_PER_BUCKET_LOG2 bits should be zero */
    hash_table.hash_mask =
        (n_slot - 1) & (0xfffffffffffffffful << N_SLOT_PER_BUCKET_LOG2);

    /* alloc table */
    hash_table.table = _hashtable_alloc(n_slot);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    for (uint64_t i = 0; i < n_slot / N_SLOT_PER_BUCKET; i++) {
        SET_BUCKET_MAGIC(hash_table.table + i * N_SLOT_PER_BUCKET);
    }
#endif

    hash_table_initialized = true;

    log_info("create hash table of %" PRIu64 " entries %" PRIu64 " buckets",
        n_slot, n_slot >> N_SLOT_PER_BUCKET_LOG2);
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

//static inline uint64_t
//_get_hv_murmur3(const char *key, size_t klen)
//{
//    uint64_t hv[2];
//
//    hash_murmur3_128_x64(key, klen, murmur3_iv, hv);
//
//    return hv[0];
//}


/**
 * insert an item into hash table
 * insert has two steps, insert and possibly delete
 * insert and delete must be completed in the same pass (atomic or locked),
 * otherwise it cannot guarantee correctness
 *
 * procedure:
 * scan through all slots of the head bucket,
 * 1. if we found the item, replace with new item
 * 2. if we found an empty slot first, we store new item in empty slots and
 *      2-1. remove the old item if the old item is in the head bucket,
 *      2-2. if we do not find it in the head bucket, we stop searching, and
 *          let the clean up to eviction time
 * 3. if old item is not found in the first bucket nor empty bucket,
 *      we continue to search
 */

void
hashtable_put(struct item *it, const uint64_t seg_id, const uint64_t offset)
{
    const char     *key = item_key(it);
    const uint32_t klen = item_nkey(it);

    uint64_t hv        = CAL_HV(key, klen);
    uint64_t tag       = CAL_TAG_FROM_HV(hv);
    uint64_t *head_bkt = GET_BUCKET(hv);
    uint64_t *bkt      = head_bkt;

    INCR(seg_metrics, hash_insert);

    /* 12-bit tag, 8-bit counter,
     * 24-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info, insert_item_info;
    insert_item_info = _build_item_info(tag, seg_id, offset);

    lock(head_bkt);

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(head_bkt);
    int n_item_slot;
    do {
        /* the last slot will be a pointer to the next
         * bucket if there is next bucket */
        n_item_slot = bkt_chain_len > 1 ?
                      N_SLOT_PER_BUCKET - 1 :
                      N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (bkt == head_bkt && i == 0) {
                /* the first slot of the head bucket is bucket into */
                continue;
            }

            item_info = bkt[i];
            if (GET_TAG(item_info) != tag) {
                if (insert_item_info != 0 && item_info == 0) {
                    /* store item info in the first empty slot */
                    bkt[i] = insert_item_info;
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
             * read and write 8-byte on x86 is always atomic */
            bkt[i] = insert_item_info;
            insert_item_info = 0;

            /* now mark the old item as deleted, update stat */
            _item_free(item_info, false);

            goto finish;
        }

        if (insert_item_info == 0) {
            /* item has been inserted, do not check next bucket to delete
             * old item, the info will be gc when item is evicted */
            goto finish;
        }

        /* if there are overflown buckets, we continue to check */
        bkt_chain_len -= 1;
        if (bkt_chain_len > 0) {
            bkt = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
        }
    } while (bkt_chain_len > 0);

    /* we have searched every bucket, but have not found the old item
     * nor inserted new item - so we need to allocate a new array,
     * this is very rare */
    INCR(seg_metrics, hash_bucket_alloc);

    uint64_t *new_bkt = cc_zalloc(sizeof(uint64_t) * N_SLOT_PER_BUCKET);
    /* move the last item from last bucket to new bucket */
    new_bkt[0] = bkt[N_SLOT_PER_BUCKET - 1];
    new_bkt[1] = insert_item_info;
    insert_item_info = 0;

    /* Q(juncheng: what is the purpose of fence */
    __atomic_thread_fence(__ATOMIC_RELEASE);

    bkt[N_SLOT_PER_BUCKET - 1] = (uint64_t) new_bkt;

    INCR_BUCKET_CHAIN_LEN(head_bkt);
    log_verb("increase bucket chain to len %d", GET_BUCKET_CHAIN_LEN(head_bkt));
    /* this is for debugging, chain length in production should not so large */
    ASSERT(GET_BUCKET_CHAIN_LEN(head_bkt) <= 16);

    finish:
    ASSERT(insert_item_info == 0);
    unlock_and_update_cas(head_bkt);
}

bool
hashtable_delete(const struct bstring *key)
{
    INCR(seg_metrics, hash_remove);

    bool     deleted = false;
    uint64_t item_info;

    uint64_t hv        = CAL_HV(key->data, key->len);
    uint64_t tag       = CAL_TAG_FROM_HV(hv);
    uint64_t *head_bkt = GET_BUCKET(hv);
    uint64_t *bkt      = head_bkt;

    lock(head_bkt);

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(head_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot =
            bkt_chain_len > 0 ?
            N_SLOT_PER_BUCKET - 1 :
            N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (bkt == head_bkt && i == 0) {
                continue;
            }

            item_info = bkt[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(key->data, key->len, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            /* found the item, now delete */
            /* if this is the first and most up-to-date hash table entry
             * we need to mark tombstone, this is for recovery */
            _item_free(item_info, !deleted);
            bkt[i] = 0;

            deleted = true;
        }
        bkt_chain_len -= 1;
        bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    unlock(head_bkt);
    return deleted;
}

/*
 * the difference between delete and evict is that
 * delete needs to mark tombstone on the most recent object,
 * evict: if the item being evicted
 *      is the most recent version (not updated),
 *          evict needs to mark tombstone on the second most recent object
 *      is not the up-to-date version
 *          evict does not need to mark tombstone
 *
 * The decision on tombstone is used for recovery, normal usage does not need
 * to mark tombstone, while tombstone is used to find out which an object is
 * an up-to-date object
 *
 */
bool
hashtable_evict(const char *oit_key, const uint32_t oit_klen,
                const uint64_t seg_id, const uint64_t offset)
{
    INCR(seg_metrics, hash_evict);

    uint64_t hv         = CAL_HV(oit_key, oit_klen);
    uint64_t tag        = CAL_TAG_FROM_HV(hv);
    uint64_t *first_bkt = GET_BUCKET(hv);
    uint64_t *bkt       = first_bkt;

    uint64_t item_info;
    uint64_t oit_info   = _build_item_info(tag, seg_id, offset);

    bool first_match = true, item_outdated = true, fount_oit = false;

    lock(first_bkt);

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot =
            bkt_chain_len > 0 ? N_SLOT_PER_BUCKET - 1 : N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (bkt == first_bkt && i == 0) {
                continue;
            }

            item_info = CLEAR_FREQ(bkt[i]);
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(oit_key, oit_klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }

            if (first_match) {
                if (oit_info == item_info) {
                    _item_free(item_info, false);
                    bkt[i] = 0;
                    item_outdated = false;
                    fount_oit     = true;
                }
                first_match = false;
                continue;
            }
            else {
                /* not first match, delete hash table entry,
                 * mark tombstone only when oit is the most up-to-date entry */
                if (!fount_oit && item_info == oit_info) {
                    fount_oit = true;
                }

                _item_free(bkt[i], !item_outdated);
                bkt[i] = 0;
            }
        }
        bkt_chain_len -= 1;
        bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    unlock(first_bkt);

    return fount_oit;
}

/**
 * delete a specific item from the hash table
 * this is needed because an updated object can have its old version
 * in object store and hash table, hashtable_delete_it deletes the specific
 * object, for example, delete an outdated object while not affecting the
 * up-to-date object
 *
 */
bool
hashtable_delete_it(struct item *it,
                    const uint64_t seg_id, const uint64_t offset)
{
    INCR(seg_metrics, hash_remove_it);

    uint64_t hv         = CAL_HV(item_key(it), it->klen);
    uint64_t tag        = CAL_TAG_FROM_HV(hv);
    uint64_t *first_bkt = GET_BUCKET(hv);
    uint64_t *bkt       = first_bkt;

    uint64_t item_info;
    uint64_t oit_info   = _build_item_info(tag, seg_id, offset);
    bool found_oit = false;

    lock(first_bkt);

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot =
            bkt_chain_len > 0 ? N_SLOT_PER_BUCKET - 1 : N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            item_info = CLEAR_FREQ(bkt[i]);
            if (oit_info == CLEAR_FREQ(bkt[i])) {
                _item_free(item_info, false);
                bkt[i] = 0;
                found_oit = true;
                break;
            }
        }
        bkt_chain_len -= 1;
        bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    unlock(first_bkt);

    return found_oit;
}

struct item *
hashtable_get(const char *key, const uint32_t klen,
              int32_t *seg_id,
              uint64_t *cas)
{
    INCR(seg_metrics, hash_lookup);

    uint64_t    hv         = CAL_HV(key, klen);
    uint64_t    tag        = CAL_TAG_FROM_HV(hv);
    uint64_t    *first_bkt = GET_BUCKET(hv);
    uint64_t    *bkt       = first_bkt;
    uint64_t    offset;
    struct item *it;

    uint64_t item_info;

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;

    //    uint64_t curr_ts = (uint64_t) time_proc_sec() & 0xfffful;
    uint64_t curr_ts = ((uint64_t) time_proc_sec()) & PROC_TS_MASK;
    if (curr_ts != GET_TS(first_bkt)) {
        /* clear the indicator of all items in the bucket that
         * the frequency has increased in curr sec */
        lock(first_bkt);

        if (curr_ts != GET_TS(first_bkt)) {
            *first_bkt =
                ((*first_bkt) & (~TS_MASK)) | (curr_ts << TS_BIT_SHIFT);
            do {
                n_item_slot = bkt_chain_len > 0 ?
                              N_SLOT_PER_BUCKET - 1 :
                              N_SLOT_PER_BUCKET;
                for (int i = 0; i < n_item_slot; i++) {
                    if (bkt == first_bkt && i == 0) {
                        continue;
                    }
                    /* clear the bit */
                    bkt[i] = bkt[i] & CLEAR_FREQ_SMOOTH_MASK;
                }
                bkt_chain_len -= 1;
                bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
            } while (bkt_chain_len >= 0);
        }

        unlock(first_bkt);

        /* reset bucket and bucket chain length */
        bkt           = first_bkt;
        bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    }

    /* try to find the item in the hash table */
    do {
        n_item_slot = bkt_chain_len > 0 ?
                      N_SLOT_PER_BUCKET - 1 :
                      N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (bkt == first_bkt && i == 0) {
                continue;
            }

            item_info = bkt[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(key, klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            if (cas) {
                *cas = GET_CAS(first_bkt);
            }

            *seg_id = GET_SEG_ID(item_info);
            offset = GET_OFFSET(item_info);

            it = (struct item *) (heap.base + heap.seg_size * *seg_id + offset);

            /* item found, try to update the frequency */
            uint64_t freq = GET_FREQ(item_info);
            if (freq < 127) {
                /* counter caps at 127 */
                if (freq <= 16 || prand() % freq == 0) {
                    /* increase frequency by 1
                     * if freq <= 16 or with prob 1/freq */
                    freq = ((freq + 1) | 0x80ul) << FREQ_BIT_SHIFT;
                }
                else {
                    /* we do not increase frequency, but mark that
                     * we have already tried at current sec */
                    freq = (freq | 0x80ul) << FREQ_BIT_SHIFT;
                }
                /* there can be benign data race where other items in the same
                 * hash bucket increases it frequency, but it is OK */
                lock(first_bkt);
                if (bkt[i] == item_info) {
                    /* make sure it is not updated by other threads */
                    bkt[i] = (item_info & (~FREQ_MASK)) | freq;
                }
                unlock(first_bkt);
            }
            /* done frequency update section */

            return it;
        }
        bkt_chain_len -= 1;
        bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    return NULL;
}

/**
 * get but not increase item frequency
 *
 **/
struct item *
hashtable_get_no_freq_incr(const char *key, const uint32_t klen,
                           int32_t *seg_id,
                           uint64_t *cas)
{
    uint64_t    hv         = CAL_HV(key, klen);
    uint64_t    tag        = CAL_TAG_FROM_HV(hv);
    uint64_t    *first_bkt = GET_BUCKET(hv);
    uint64_t    *bkt       = first_bkt;
    uint64_t    offset;
    struct item *it;

    /* 16-bit tag, 28-bit seg id, 20-bit offset (in the unit of 8-byte) */
    uint64_t item_info;

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot = bkt_chain_len > 0 ?
                      N_SLOT_PER_BUCKET - 1 :
                      N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (bkt == first_bkt && i == 0) {
                continue;
            }

            item_info = bkt[i];
            if (GET_TAG(item_info) != tag) {
                continue;
            }
            /* a potential hit */
            if (!_same_item(key, klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }
            if (cas) {
                *cas = GET_CAS(first_bkt);
            }

            *seg_id = GET_SEG_ID(item_info);
            offset = GET_OFFSET(item_info);
            it     = (struct item *) (heap.base + heap.seg_size * (*seg_id)
                + offset);

            return it;
        }
        bkt_chain_len -= 1;
        bkt        = (uint64_t *) (bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    return NULL;
}

/**
 * get item frequency
 *
 **/
int
hashtable_get_it_freq(const char *it_key, const uint32_t it_klen,
                      const uint64_t seg_id, const uint64_t offset)
{
    uint64_t hv  = CAL_HV(it_key, it_klen);
    uint64_t tag = CAL_TAG_FROM_HV(hv);

    uint64_t *first_bkt        = GET_BUCKET(hv);
    uint64_t *curr_bkt         = first_bkt;
    uint64_t curr_item_info;
    uint64_t item_info_to_find = _build_item_info(tag, seg_id, offset);
    int      freq              = 0;

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot = bkt_chain_len > 0 ?
                      N_SLOT_PER_BUCKET - 1 :
                      N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (curr_bkt == first_bkt && i == 0) {
                continue;
            }

            curr_item_info = CLEAR_FREQ(curr_bkt[i]);
            if (GET_TAG(curr_item_info) != tag) {
                continue;
            }

            if (curr_item_info == item_info_to_find) {
                freq = GET_FREQ(curr_bkt[i]) & 0x7Ful;
                return freq;
            }

            /* a potential hit */
            if (!_same_item(it_key, it_klen, curr_item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }

            /* the item to find is outdated */
            return 0;

        }
        bkt_chain_len -= 1;
        curr_bkt   = (uint64_t *) (curr_bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    return 0;
}


/*
 * relink is used when the item is moved from one segment to another
 *
 * a few caveats
 *  item being relinked could be outdated, in which case we should not relink
 *
 * TODO(jason): it might be better not clear those old entries?
 */
bool
hashtable_relink_it(const char *oit_key, const uint32_t oit_klen,
                    const uint64_t old_seg_id, const uint64_t old_offset,
                    const uint64_t new_seg_id, const uint64_t new_offset)
{
    INCR(seg_metrics, hash_relink);

    uint64_t hv         = CAL_HV(oit_key, oit_klen);
    uint64_t tag        = CAL_TAG_FROM_HV(hv);
    uint64_t *first_bkt = GET_BUCKET(hv);
    uint64_t *curr_bkt  = first_bkt;
    uint64_t item_info;
    bool item_outdated = true, first_match = true;

    uint64_t oit_info = _build_item_info(tag, old_seg_id, old_offset);
    uint64_t nit_info = _build_item_info(tag, new_seg_id, new_offset);

    lock(first_bkt);

    int bkt_chain_len = GET_BUCKET_CHAIN_LEN(first_bkt) - 1;
    int n_item_slot;
    do {
        n_item_slot = bkt_chain_len > 0 ?
                      N_SLOT_PER_BUCKET - 1 :
                      N_SLOT_PER_BUCKET;

        for (int i = 0; i < n_item_slot; i++) {
            if (curr_bkt == first_bkt && i == 0) {
                continue;
            }

            item_info = CLEAR_FREQ(curr_bkt[i]);
            if (GET_TAG(item_info) != tag) {
                continue;
            }

            /* a potential hit */
            if (!_same_item(oit_key, oit_klen, item_info)) {
                INCR(seg_metrics, hash_tag_collision);
                continue;
            }

            if (first_match) {
                if (oit_info == item_info) {
                    /* item is not outdated */
                    curr_bkt[i] = nit_info;
                    item_outdated = false;
                }
                first_match = false;
                continue;
            }

            /* not first match, delete */
            _item_free(curr_bkt[i], false);
            curr_bkt[i] = 0;
        }
        bkt_chain_len -= 1;
        curr_bkt   = (uint64_t *) (curr_bkt[N_SLOT_PER_BUCKET - 1]);
    } while (bkt_chain_len >= 0);

    unlock(first_bkt);

    return !item_outdated;
}

void
hashtable_stat(int *item_cnt_ptr, int *bucket_cnt_ptr)
{
#define BUCKET_HEAD(idx) (&hash_table.table[(idx) * N_SLOT_PER_BUCKET])

    *item_cnt_ptr   = 0;
    *bucket_cnt_ptr = 0;

    int n_item_slots; /* the number of used slot in current bucket */
    int bkt_chain_len; /* the number of buckets in current bucket chain */

    uint64_t item_info, *head_bkt, *curr_bkt;

    for (uint64_t bucket_idx = 0;
         bucket_idx < HASHSIZE(hash_table.hash_power - N_SLOT_PER_BUCKET_LOG2);
         bucket_idx++) {

        head_bkt      = curr_bkt = BUCKET_HEAD(bucket_idx);
        bkt_chain_len = GET_BUCKET_CHAIN_LEN(head_bkt);
        *bucket_cnt_ptr += bkt_chain_len;
        do {
            n_item_slots = bkt_chain_len > 1 ?
                           N_SLOT_PER_BUCKET - 1 :
                           N_SLOT_PER_BUCKET;

            for (int i = 0; i < n_item_slots; i++) {
                /* this is bucket info */
                if (curr_bkt == head_bkt && i == 0) {
                    continue;
                }

                item_info = curr_bkt[i];
                if (item_info != 0) {
                    *item_cnt_ptr += 1;
                }
            }
            bkt_chain_len -= 1;
            if (bkt_chain_len > 0) {
                curr_bkt = (uint64_t *) (curr_bkt[N_SLOT_PER_BUCKET - 1]);
            }
        } while (bkt_chain_len > 0);
    }

    log_info("hashtable %d items, %d buckets", *item_cnt_ptr, *bucket_cnt_ptr);

#undef BUCKET_HEAD
}


void
scan_hashtable_find_seg(int32_t target_seg_id)
{
#define BUCKET_HEAD(idx) (&hash_table.table[(idx) * N_SLOT_PER_BUCKET])
    /* expensive debug */

    int         bkt_chain_len;
    uint64_t    item_info;
    uint64_t    *head_bkt, *curr_bkt;
    int         n_item_slot;
    uint64_t    seg_id;
    uint64_t    offset;
    struct item *it;

    int n_bkt_in_table =
            HASHSIZE(hash_table.hash_power - N_SLOT_PER_BUCKET_LOG2);

    for (uint64_t bucket_idx = 0; bucket_idx < n_bkt_in_table; bucket_idx++) {
        curr_bkt      = head_bkt = BUCKET_HEAD(bucket_idx);
        bkt_chain_len = GET_BUCKET_CHAIN_LEN(head_bkt);
        do {
            n_item_slot = bkt_chain_len >= 1 ?
                          N_SLOT_PER_BUCKET - 1 :
                          N_SLOT_PER_BUCKET;

            for (int i = 0; i < n_item_slot; i++) {
                if (curr_bkt == head_bkt && i == 0) {
                    continue;
                }

                item_info = curr_bkt[i];

                if (item_info == 0) {
                    continue;
                }

                seg_id = ((item_info & SEG_ID_MASK) >> SEG_ID_BIT_SHIFT);
                if (target_seg_id == seg_id) {
                    offset = (item_info & OFFSET_MASK) << OFFSET_UNIT_IN_BIT;
                    it =
                        (struct item *) (heap.base + heap.seg_size * seg_id +
                            offset);
                    log_warn("find item (%.*s) klen %d on seg %d offset %d, "
                        "item_info %lu, slot %d, bkt_len %d, bkt_len left %d",
                        it->klen, item_key(it), it->klen, seg_id, offset,
                        item_info, i,
                        GET_BUCKET_CHAIN_LEN(head_bkt), bkt_chain_len);
                    ASSERT(0);
                }
            }
            bkt_chain_len -= 1;
            curr_bkt   = (uint64_t *) (curr_bkt[N_SLOT_PER_BUCKET - 1]);
        } while (bkt_chain_len > 0);
    }

#undef BUCKET_HEAD
}
















