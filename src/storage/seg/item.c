#include "item.h"
#include "hashtable2.h"
#include "seg.h"
#include "ttlbucket.h"

#include <cc_debug.h>

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>

extern proc_time_i flush_at;
extern struct hash_table *hash_table;
extern seg_metrics_st *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];

#define SANITY_CHECK(it)                                                       \
    do {                                                                       \
        ASSERT(it->magic == ITEM_MAGIC);                                       \
        ASSERT(seg_get_data_start(it->seg_id) != NULL);                        \
        ASSERT(*(uint64_t *)(seg_get_data_start(it->seg_id)) == SEG_MAGIC);    \
    } while (0)


/*
 * verify the integrity of segments, items and hashtable
 */
static inline void
_verify_integrity(void)
{
    uint32_t seg_id;
    struct seg *seg;
    uint8_t *seg_data, *curr;
    struct item *it, *it2;

    for (seg_id = 0; seg_id < heap.nseg; seg_id++) {
        seg = &heap.segs[seg_id];
        ASSERT(seg->seg_id == seg_id);
        seg_data = curr = seg_get_data_start(seg_id);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
        ASSERT(*(uint64_t *)(curr) == SEG_MAGIC);
        curr += sizeof(uint64_t);
#endif
        while (curr - seg_data < seg->write_offset) {
            it = (struct item *)curr;
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
            ASSERT(it->magic == ITEM_MAGIC);
#endif

            struct bstring key = {.data = item_key(it), .len = item_nkey(it)};
            it2 = item_get(&key, NULL, true);
            if (it2 != NULL) {
                /* item might be deleted */
                ASSERT(item_nkey(it) == item_nkey(it2));
                ASSERT(item_nval(it) == item_nval(it2));
                cc_memcmp(item_key(it), item_key(it2), item_nkey(it));
                cc_memcmp(item_val(it), item_val(it2), item_nval(it));
                item_release(it2);
            }

            curr += item_ntotal(it);
        }
    }
}

/*
 * Allocate an item. We allocate an item by consuming the next free item
 * from slab of the item's slab class.
 *
 * On success we return the pointer to the allocated item.
 */
static struct item *
_item_alloc(uint32_t sz, delta_time_i ttl, uint32_t *seg_id)
{
    uint16_t ttl_bucket_idx = find_ttl_bucket_idx(ttl);
    struct item *it = ttl_bucket_reserve_item(ttl_bucket_idx, sz, seg_id);

    if (it == NULL) {
        INCR(seg_metrics, item_alloc_ex);
        log_error("error alloc it %p of size %" PRIu32 " ttl %" PRIu32
                  " (bucket %" PRIu16 ") in seg %" PRIu32,
                it, sz, ttl, ttl_bucket_idx);

        return NULL;
    }

    struct seg *curr_seg = &heap.segs[*seg_id];

    if (!seg_w_ref(*seg_id)) {
        /* should be very rare -
         * TTL is shorter than the segment write time or
         * something is wrong and the eviction algorithm picked this segment
         *
         * roll back the seg stat for avoid inconsistency at eviction
         **/

        INCR(seg_metrics, item_alloc_ex);

        __atomic_sub_fetch(&curr_seg->write_offset, sz, __ATOMIC_SEQ_CST);

        log_warn("allocated item is about to be evicted, seg info ");
        seg_print(*seg_id);

        /* TODO(jason): maybe we should retry here */
        return NULL;
    }

    uint32_t occupied_size = __atomic_add_fetch(
            &(curr_seg->occupied_size), sz, __ATOMIC_SEQ_CST);
    ASSERT(occupied_size <= heap.seg_size);

    __atomic_add_fetch(&curr_seg->n_item, 1, __ATOMIC_SEQ_CST);

    INCR(seg_metrics, item_alloc);
    INCR(seg_metrics, item_curr);
    INCR_N(seg_metrics, item_curr_bytes, sz);
    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);

    log_vverb("alloc it %p of size %" PRIu32 " ttl %" PRIu32 " (bucket %" PRIu16
              ") in seg %" PRIu32,
            it, sz, ttl, ttl_bucket_idx, *seg_id);

    return it;
}


/* insert or update */
void
item_insert(struct item *it)
{
    /* calculate seg_id from it address */
    uint32_t seg_id = (((uint8_t *)it) - heap.base) / heap.seg_size;
    uint32_t offset = ((uint8_t *)it) - heap.base - heap.seg_size * seg_id;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(*(uint64_t *)(seg_get_data_start(seg_id)) == SEG_MAGIC);
#endif

    hashtable_put(it, (uint64_t)seg_id, (uint64_t)offset);

    seg_w_deref(seg_id);

    log_verb("insert_or_update it %p (%.*s) of key size %u, val size %u, "
             "total size %zu in seg %d, "
             "seg write-offset %d, occupied size %d",
            it, item_nkey(it), item_key(it), item_nkey(it), item_nval(it),
            item_ntotal(it), seg_id,
            __atomic_load_n(&heap.segs[seg_id].write_offset, __ATOMIC_RELAXED),
            __atomic_load_n(
                    &heap.segs[seg_id].occupied_size, __ATOMIC_RELAXED));
}

/**
 * find the key in the cache and return,
 * return NULL if not in the cache (never added or evicted, or expired)
 *
 */
struct item *
item_get(const struct bstring *key, uint64_t *cas, bool incr_ref)
{
    struct item *it;
    struct seg *seg;
    uint32_t seg_id;

    it = hashtable_get(key->data, key->len, &seg_id, cas);
    if (it == NULL) {
        log_vverb("get it '%.*s' not found", key->len, key->data);
        return NULL;
    }

    seg = &heap.segs[seg_id];

    if (seg_expired(seg_id)) {
        log_warn("item_get found expired item on seg %"
                PRIu32 ", background thread cannot catch up", seg_id);
        seg_print(seg_id);
        return NULL;
    }

    //    if (__atomic_load_n(&seg->locked, __ATOMIC_SEQ_CST)) {
    //        log_verb("get it %.*s not available because seg is locked for "
    //                 "eviction/expiration",
    //                item_nkey(it), item_key(it));
    //
    //        return NULL;
    //    }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(*(uint64_t *)(seg_get_data_start(seg_id)) == SEG_MAGIC);
#endif

    if (incr_ref) {
        /* a seg can be locked between last check and incr refcount, it is fine
         */
        __atomic_fetch_add(&seg->r_refcount, 1, __ATOMIC_SEQ_CST);
    }

    log_vverb("get it key %.*s", key->len, key->data);

    return it;
}

void
item_release(struct item *it)
{
    uint32_t seg_id = (((uint8_t *)it) - heap.base) / heap.seg_size;
    seg_r_deref(seg_id);
}

static void
_item_define(struct item *it, const struct bstring *key,
        const struct bstring *val, uint8_t olen)
{
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    it->magic = ITEM_MAGIC;
#endif

    it->olen = olen;
    cc_memcpy(item_key(it), key->data, key->len);
    it->klen = key->len;
    if (val != NULL) {
        cc_memcpy(item_val(it), val->data, val->len);
    }
    it->vlen = (val == NULL) ? 0 : val->len;
}

item_rstatus_e
item_reserve(struct item **it_p, const struct bstring *key,
        const struct bstring *val, uint32_t vlen, uint8_t olen,
        proc_time_i expire_at)
{
    struct item *it;
    uint32_t seg_id;
    delta_time_i ttl = expire_at - time_proc_sec();
    size_t sz = item_size(key->len, vlen, olen);

    if (sz > heap.seg_size) {
        *it_p = NULL;
        return ITEM_EOVERSIZED;
    }

    if ((it = _item_alloc(sz, ttl, &seg_id)) == NULL) {
        log_warn("item reservation failed");
        *it_p = NULL;
        return ITEM_ENOMEM;
    }

    _item_define(it, key, val, olen);

    *it_p = it;

    log_verb("reserve it %p (%.*s) of size %u ttl %d in seg %d (my offset %d "
             "write offset %d)",
            it, it->klen, item_key(it), item_ntotal(it), ttl, seg_id,
            (uint8_t *)it - seg_get_data_start(seg_id),
            __atomic_load_n(&heap.segs[seg_id].write_offset, __ATOMIC_SEQ_CST));


    return ITEM_OK;
}


void
item_backfill(struct item *it, const struct bstring *val)
{
    ASSERT(it != NULL);

    cc_memcpy(item_val(it) + it->vlen, val->data, val->len);

    it->vlen += val->len;

    log_verb("backfill it %p (%.*s) with %" PRIu32 " bytes, now total %" PRIu16,
            it, it->klen, item_key(it), val->len, it->vlen);
}

/* TODO(jason): better change the interface to use bstring key and do item_get
 * inside function, so that we can manage refcount within function */

item_rstatus_e
item_incr(uint64_t *vint, struct item *it, uint64_t delta)
{
    /* do not incr refcount since we have already called item_get */
    if (it->is_num) {
        *vint = *(uint64_t *)item_val(it) + delta;
    } else {
        struct bstring vstr = {.data = (char *)item_val(it), .len = it->vlen};
        if (bstring_atou64(vint, &vstr) == CC_OK) {
            it->is_num = true;
            *vint = *vint + delta;
        } else {
            return ITEM_ENAN;
        }
    }

    *(uint64_t *)item_val(it) = *vint;
    return ITEM_OK;
}

item_rstatus_e
item_decr(uint64_t *vint, struct item *it, uint64_t delta)
{
    if (it->is_num) {
        if (*(uint64_t *)item_val(it) >= delta) {
            *vint = *(uint64_t *)item_val(it) - delta;
        } else {
            *vint = 0;
        }
    } else {
        struct bstring vstr = {.data = (char *)item_val(it), .len = it->vlen};
        if (bstring_atou64(vint, &vstr) == CC_OK) {
            it->is_num = true;
            *vint = *vint - delta;
        } else {
            return ITEM_ENAN;
        }
    }
    *(uint64_t *)item_val(it) = *vint;
    return ITEM_OK;
}


bool
item_delete(const struct bstring *key)
{
    log_verb("delete it (%.*s)", key->len, key->data);
    return hashtable_delete(key->data, key->len, true);
}

// bool
// item_evict(const char *oit_key, const uint32_t oit_klen,
//               const uint32_t seg_id, const uint32_t offset)
//{
//    bool in_cache = hashtable_delete_it(oit_key, oit_klen, seg_id, offset);
//
//    log_verb("delete it %.*s from seg %d in_cache %d", oit_klen, oit_key,
//            seg_id, in_cache);
//
//    return in_cache;
//}

void
item_flush(void)
{
    time_update();
    flush_at = time_proc_sec();
    log_info("all keys flushed at %" PRIu32, flush_at);
}
