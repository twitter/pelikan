#include "item.h"
#include "hashtable.h"
#include "seg.h"
#include "ttlbucket.h"

#include <cc_debug.h>

#include <inttypes.h>
#include <stdio.h>

extern proc_time_i flush_at;
extern struct hash_table *hash_table;
extern seg_metrics_st *seg_metrics;
extern seg_perttl_metrics_st perttl[MAX_N_TTL_BUCKET];


static struct item *
_item_alloc(uint32_t sz, int32_t ttl_bucket_idx, int32_t *seg_id)
{
    struct item *it = ttl_bucket_reserve_item(ttl_bucket_idx, sz, seg_id);

    if (it == NULL) {
        INCR(seg_metrics, item_alloc_ex);
        log_error("error alloc it %p of size %" PRIu32
                  " (bucket %" PRIu16 ") in seg %" PRIu32,
                it, sz, ttl_bucket_idx, seg_id);

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

        /* done in ttl bucket */
//        __atomic_sub_fetch(&curr_seg->write_offset, sz, __ATOMIC_RELEASE);

        log_warn("allocated item is not accessible (seg is expiring or "
                 "being evicted), ttl %d", curr_seg->ttl);

        /* TODO(jason): maybe we should retry here */
        return NULL;
    }


    INCR(seg_metrics, item_alloc);

    log_vverb("alloc it %p of size %" PRIu32 " in TTL bucket %" PRIu16
              " and seg %" PRIu32,
            it, sz, ttl_bucket_idx, *seg_id);

    return it;
}

static void
_item_define(struct item *it, const struct bstring *key,
             const struct bstring *val, uint8_t olen,
             int32_t seg_id, int32_t ttl_bucket_idx, size_t sz)
{
#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    it->magic = ITEM_MAGIC;
#endif

    it->olen = olen;
    cc_memcpy(item_key(it), key->data, key->len);
    it->klen = key->len;
//    if (val != NULL) {
//        cc_memcpy(item_val(it), val->data, val->len);
//    }
    it->vlen = (val == NULL) ? 0 : val->len;


    struct seg *curr_seg = &heap.segs[seg_id];

    int32_t occupied_size = __atomic_add_fetch(
            &(curr_seg->occupied_size), sz, __ATOMIC_RELAXED);
    ASSERT(occupied_size <= heap.seg_size);

    __atomic_add_fetch(&curr_seg->n_item, 1, __ATOMIC_RELAXED);

    INCR(seg_metrics, item_curr);
    INCR_N(seg_metrics, item_curr_bytes, sz);
    PERTTL_INCR(ttl_bucket_idx, item_curr);
    PERTTL_INCR_N(ttl_bucket_idx, item_curr_bytes, sz);
}

/* insert or update */
void
item_insert(struct item *it)
{
    /* calculate seg_id from it address */
    int32_t seg_id = (((uint8_t *)it) - heap.base) / heap.seg_size;
    int32_t offset = ((uint8_t *)it) - heap.base - heap.seg_size * seg_id;

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(*(uint64_t *)(seg_get_data_start(seg_id)) == SEG_MAGIC);
#endif

    hashtable_put(it, (uint64_t)seg_id, (uint64_t)offset);

    seg_w_deref(seg_id);

    log_verb("insert it %p (%.*s) of key size %u, val size %u, "
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
    int32_t seg_id;

    it = hashtable_get(key->data, key->len, &seg_id, cas);
    if (it == NULL) {
        log_vverb("get it '%.*s' not found", key->len, key->data);

        return NULL;
    }

    seg = &heap.segs[seg_id];

    if (!seg_accessible(seg_id)) {
        log_verb("get it '%.*s' not accessible/expired", key->len, key->data);

        return NULL;
    }

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    ASSERT(it->magic == ITEM_MAGIC);
#endif

    if (incr_ref) {
        __atomic_fetch_add(&seg->r_refcount, 1, __ATOMIC_RELAXED);
    }

#ifdef TRACK_ADVANCED_STAT
    __atomic_fetch_add(&seg->n_hit, 1, __ATOMIC_RELAXED);
//    if (time_proc_sec() - seg->create_at >= ACTIVE_ITEM_START_REC_TIME) {
        int32_t idx = (uint32_t) (((uint8_t*)(it)-seg_get_data_start(seg_id))) >> 3u;
        ASSERT(idx < 131072);
        if (seg->active_obj[idx] == 0) {
            seg->n_active += 1;
            seg->n_active_byte += item_ntotal(it);
        }
        if (__atomic_load_n(&seg->active_obj[idx], __ATOMIC_RELAXED) < (1 << 16) -1)
            __atomic_fetch_add(&seg->active_obj[idx], 1, __ATOMIC_RELAXED);
//    }
//    log_warn("get %.*s idx %d - %d seg %d", it->klen, item_key(it), idx,
//            seg->active_obj[idx], seg->seg_id);
#endif

    log_vverb("get it key %.*s", key->len, key->data);

    return it;
}

void
item_release(struct item *it)
{
    int32_t seg_id = (((uint8_t *)it) - heap.base) / heap.seg_size;
    seg_r_deref(seg_id);
}


item_rstatus_e
item_reserve(struct item **it_p, const struct bstring *key,
        const struct bstring *val, uint32_t vlen, uint8_t olen,
        proc_time_i expire_at)
{
    struct item *it;
    int32_t seg_id;
    delta_time_i ttl = expire_at - time_proc_sec();

    if (ttl <= 0) {
        log_warn("reserve_item (%.*s) ttl %" PRId32, key->len, key->data, ttl);
    }

    int32_t ttl_bucket_idx = find_ttl_bucket_idx(ttl);
    size_t sz = item_size(key->len, vlen, olen);

    if (sz > heap.seg_size) {
        *it_p = NULL;
        return ITEM_EOVERSIZED;
    }

    if ((it = _item_alloc(sz, ttl_bucket_idx, &seg_id)) == NULL) {
        log_warn("item reservation failed");
        *it_p = NULL;
        return ITEM_ENOMEM;
    }

    _item_define(it, key, val, olen, seg_id, ttl_bucket_idx, sz);

    *it_p = it;

    log_verb("reserve it %p (%.*s) of size %u ttl %d in seg %d (my offset %d "
             "write offset %d)",
            it, it->klen, item_key(it), item_ntotal(it), ttl, seg_id,
            (uint8_t *)it - seg_get_data_start(seg_id),
            __atomic_load_n(&heap.segs[seg_id].write_offset, __ATOMIC_RELAXED));

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
            it->vlen = sizeof(uint64_t);
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
            it->vlen = sizeof(uint64_t);
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
    return hashtable_delete(key->data, key->len);
}

void
item_flush(void)
{
    time_update();
    flush_at = time_proc_sec();
    log_info("all keys flushed at %" PRIu32, flush_at);
}
