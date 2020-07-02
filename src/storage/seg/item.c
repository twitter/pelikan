#include "item.h"
#include "hashtable.h"
#include "seg.h"
#include "ttlbucket.h"

#include <cc_debug.h>

#include <stdio.h>
#include <stdlib.h>

extern proc_time_i flush_at;
extern struct hash_table *hash_table;
extern seg_metrics_st *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_TTL_BUCKET];
// extern bool use_cas;

static inline bool
item_expired(struct item *it)
{
    struct seg *seg = item_to_seg(it);
    /* seg->locked means being evicted, should not read it */
    uint8_t locked = __atomic_load_n(&seg->locked, __ATOMIC_RELAXED);
    bool expired = locked || seg->ttl + seg->create_at < time_proc_sec();
    expired = expired || seg->create_at <= flush_at;

    if (expired && !locked) {
        seg_rm_expired_seg(seg->seg_id);
    }
    return expired;
}


static inline void
_item_hdr_init(struct item *it)
{
#if CC_ASSERT_PANIC == 1 || CC_ASSERT_LOG == 1
    it->magic = ITEM_MAGIC;
#endif
}


/*
 * this is only used when migrating or compacting segments
 * it assumes oit is in the hashtable, now we update hashtable entry to new loc
 */
void
item_relink(struct item *oit, struct item *nit)
{
    hashtable_delete(item_key(oit), oit->klen, hash_table, true, NULL);
    hashtable_put(nit, hash_table);
}

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
            ASSERT(it->seg_id == seg_id);

            struct bstring key = {.data = item_key(it), .len = item_nkey(it)};
            it2 = item_get(&key);
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

static inline bool _item_r_ref(struct item *it){
    struct seg *seg = &heap.segs[it->seg_id];

    if (__atomic_load_n(&seg->locked, __ATOMIC_RELAXED) == 0) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->r_refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

static inline void _item_r_deref(struct item *it) {
    struct seg *seg = &heap.segs[it->seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->r_refcount, 1, __ATOMIC_RELAXED);

    ASSERT(ref >= 0);
}

static inline bool _item_w_ref(struct item *it){
    struct seg *seg = &heap.segs[it->seg_id];

    if (__atomic_load_n(&seg->locked, __ATOMIC_RELAXED) == 0) {
        /* this does not strictly prevent race condition, but it is fine
         * because letting one reader passes when the segment is locking
         * has no problem in correctness */
        __atomic_fetch_add(&seg->w_refcount, 1, __ATOMIC_RELAXED);
        return true;
    }

    return false;
}

static inline void _item_w_deref(struct item *it) {
    struct seg *seg = &heap.segs[it->seg_id];

    uint32_t ref = __atomic_sub_fetch(&seg->w_refcount, 1, __ATOMIC_RELAXED);

    ASSERT(ref >= 0);
}

static inline void
_item_free(struct item *it)
{
    size_t sz = item_ntotal(it);
    struct seg *seg = item_to_seg(it);
    __atomic_sub_fetch(&seg->occupied_size, item_ntotal(it), __ATOMIC_RELAXED);
    __atomic_sub_fetch(&seg->n_item, 1, __ATOMIC_RELAXED);

    /* TODO(jason): what is the overhead of tracking PERTTL metric
     * consider removing the metrics since we can get them from
     * iterating over all seg headers */
    uint16_t ttl_bucket_idx = find_ttl_bucket_idx(seg->ttl);

    DECR(seg_metrics, item_curr);
    DECR_N(seg_metrics, item_curr_bytes, sz);

    PERTTL_DECR(ttl_bucket_idx, item_curr);
    PERTTL_DECR_N(ttl_bucket_idx, item_curr_bytes, sz);
}

/*
 * Allocate an item. We allocate an item by consuming the next free item
 * from slab of the item's slab class.
 *
 * On success we return the pointer to the allocated item.
 */
static struct item *
_item_alloc(uint32_t sz, delta_time_i ttl)
{
    uint16_t ttl_bucket_idx = find_ttl_bucket_idx(ttl);
    struct item *it = ttl_bucket_reserve_item(ttl_bucket_idx, sz);

    if (it == NULL) {
        INCR(seg_metrics, item_alloc_ex);
        log_error("error alloc it %p of size %" PRIu32 " ttl %" PRIu32
                 " (bucket %" PRIu16 ") in seg %" PRIu32,
                it, sz, ttl, ttl_bucket_idx, it->seg_id);

        return NULL;
    }

    if (!_item_w_ref(it)) {
        /* should be very rare -
         * TTL is shorter than the segment write time */
        INCR(seg_metrics, item_alloc_ex);
        log_error("allocated item is about to be evicted");

        return NULL;
    }

    _item_hdr_init(it);

    INCR(seg_metrics, item_alloc);
    INCR(seg_metrics, item_curr);
    INCR_N(seg_metrics, item_curr_bytes, sz);
    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);

    log_verb("alloc it %p of size %" PRIu32 " ttl %" PRIu32 " (bucket %" PRIu16
             ") in seg %" PRIu32,

            it, sz, ttl, ttl_bucket_idx, it->seg_id);

    return it;
}


/*
 * this assumes the inserted item is not in the hashtable
 */
void
item_insert(struct item *it)
{
    hashtable_put(it, hash_table);

    _item_w_deref(it);

    log_verb("insert it %p (%.*s) of size %zu"
             " in seg %" PRIu32,
            it, it->klen, item_key(it), item_ntotal(it), it->seg_id);
}

/*
 * this assumes the updated item is in the hashtable,
 * we delete the item first (update metrics), then insert into hashtable
 */
void
item_update(struct item *nit)
{
    struct item *oit;
    hashtable_delete(item_key(nit), item_nkey(nit), hash_table, false, &oit);
    _item_free(oit);

    hashtable_put(nit, hash_table);

    _item_w_deref(nit);

    log_verb("update it %p (%.*s) of size %zu"
             " in seg %" PRIu32,
            nit, nit->klen, item_key(nit), item_ntotal(nit), nit->seg_id);
}

/* insert or update */
void
item_insert_or_update(struct item *it)
{
    struct bstring key = {.data = item_key(it), .len = item_nkey(it)};
    item_delete(&key);

    hashtable_put(it, hash_table);

    _item_w_deref(it);

    log_verb("insert_or_update it %p (%.*s) of key size %" PRIu32
             ", val size %" PRIu32 ", total size %zu"
             " in seg %" PRIu32,
            it, it->klen, item_key(it), item_nkey(it), item_nval(it),
            item_ntotal(it), it->seg_id);
}


struct item *
item_check_existence(const struct bstring *key)
{
    struct item *it;

    it = hashtable_get(key->data, key->len, hash_table);
    if (it == NULL) {
        log_verb("get it '%.*s' not found", key->len, key->data);
        return NULL;
    }

    if (item_expired(it)) {
        log_verb("get it '%.*s' expired and seg nuked", key->len, key->data);

        return NULL;
    }

    return it;
}

/**
 * find the key in the cache and return,
 * return NULL if not in the cache (never added or evicted, or expired)
 *
 * incr_ref decides whether we want to increase ref_count,
 * if it is just
 */
struct item *
item_get(const struct bstring *key)
{
    struct item *it = item_check_existence(key);

    if (it == NULL) {
        return NULL;
    }

    if (_item_r_ref(it)) {
        log_vverb("get it key %.*s val %.*s", key->len, key->data, it->vlen,
                item_val(it));

        return it;
    } else {
        log_verb("get it '%.*s' seg locked", key->len, key->data);

        return NULL;
    }
}


void
item_release(struct item *it)
{
    _item_r_deref(it);
}

static void
_item_define(struct item *it, const struct bstring *key,
        const struct bstring *val, uint8_t olen)
{
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
    delta_time_i ttl = expire_at - time_proc_sec();
    size_t sz = item_size(key->len, vlen, olen);

    if (sz > heap.seg_size) {
        *it_p = NULL;
        return ITEM_EOVERSIZED;
    }

    if ((it = _item_alloc(sz, ttl)) == NULL) {
        log_warn("item reservation failed");
        *it_p = NULL;
        return ITEM_ENOMEM;
    }

    _item_define(it, key, val, olen);
    *it_p = it;

    log_verb("reserve it %p (%.*s) of size %" PRIu32 " in seg %" PRIu16, it,
            it->klen, item_key(it), item_ntotal(it), it->seg_id);

    return ITEM_OK;
}

#ifdef do_not_define
/* given an old item, recreate a new item */
item_rstatus_e
item_recreate(struct item **nit_p, struct item *oit, delta_time_i ttl,
        delta_time_i create_at)
{
    item_rstatus_e status;
    struct item *it;

    status = _item_alloc(nit_p, oit->klen, oit->vlen, oit->olen, ttl);
    if (status != ITEM_OK) {
        log_debug("item reservation failed");
        return status;
    }

    it = *nit_p;

    it->olen = oit->olen;
    if (it->olen > 0) {
        cc_memcpy(item_optional(it), item_optional(oit), oit->olen);
    }
    cc_memcpy(item_key(it), item_key(oit), oit->klen);
    it->klen = oit->klen;
    cc_memcpy(item_val(it), item_val(it), oit->vlen);
    it->vlen = oit->vlen;

    log_verb("recreate it %p (%.*s) of size %" PRIu32 " in seg %" PRIu16, it,
            it->klen, item_key(it), item_ntotal(it), it->seg_id);

    return ITEM_OK;
}
#endif

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
//            _item_r_deref(it);
            return ITEM_ENAN;
        }
    }

    *(uint64_t *)item_val(it) = *vint;
    //    seg_deref(seg);
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
//            _item_r_deref(it);
            return ITEM_ENAN;
        }
    }
    *(uint64_t *)item_val(it) = *vint;
    //    seg_deref(seg);
    return ITEM_OK;
}

// void
// item_update(struct item *it, const struct bstring *val)
//{
//    ASSERT(it->vlen <= val->len);
//
//    log_verb("in-place update it %p (%.*s) from size %"
//        PRIu32
//        " to size %"
//        PRIu32, it, it->klen, item_key(it), it->vlen, val->len);
//
//    it->vlen = val->len;
//    cc_memcpy(item_val(it), val->data, val->len);
////    item_set_cas(it);
//
//}


bool
item_delete(const struct bstring *key)
{
    struct item *it = NULL;

    bool in_cache =
            hashtable_delete(key->data, key->len, hash_table, true, &it);

    if (in_cache) {
        _item_free(it);
    }

    return in_cache;
}

bool
item_delete_it(struct item *it_to_del)
{
    bool in_cache = hashtable_delete_it(it_to_del, hash_table);

    if (in_cache) {
        _item_free(it_to_del);
    }

    return in_cache;
}

void
item_flush(void)
{
    time_update();
    flush_at = time_proc_sec();
    log_info("all keys flushed at %" PRIu32, flush_at);
}
