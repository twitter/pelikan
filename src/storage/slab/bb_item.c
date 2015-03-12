#include <storage/slab/bb_item.h>

#include <storage/slab/bb_assoc.h>

#include <stdlib.h>
#include <stdio.h>

static uint64_t cas_id;                         /* unique cas id */

/*
 * Returns the next cas id for a new item. Minimum cas value
 * is 1 and the maximum cas value is UINT64_MAX
 */
static uint64_t
_item_next_cas(void)
{
    if (use_cas) {
        return ++cas_id;
    }

    return 0ULL;
}

static bool
_item_expired(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    return (it->exptime > 0 && it->exptime < time_now()) ? true : false;
}

void
item_setup(void)
{
    log_debug("item hdr size %d", ITEM_HDR_SIZE);

    cas_id = 0ULL;
}

void
item_teardown(void)
{
}

/*
 * Get start location of item payload
 */
char *
item_data(struct item *it)
{
    char *data;

    ASSERT(it->magic == ITEM_MAGIC);

    if (it->is_raligned) {
        data = (char *)it + slab_item_size(it->id) - it->vlen;
    } else {
        data = it->end + it->klen + (it->has_cas ? sizeof(uint64_t) : 0);
    }

    return data;
}

/*
 * Get the slab that contains this item.
 */
struct slab *
item_to_slab(struct item *it)
{
    struct slab *slab;

    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(it->offset < slab_size_setting);

    slab = (struct slab *)((uint8_t *)it - it->offset);

    ASSERT(slab->magic == SLAB_MAGIC);

    return slab;
}

void
item_hdr_init(struct item *it, uint32_t offset, uint8_t id)
{
    ASSERT(offset >= SLAB_HDR_SIZE && offset < slab_size_setting);

#if CC_ASSERT_PANIC == 1 || CC_ASSERT_LOG == 1
    it->magic = ITEM_MAGIC;
#endif
    it->offset = offset;
    it->id = id;
    it->refcount = 0;
    it->is_linked = it->has_cas = it->in_freeq = it->is_raligned = 0;
}

uint8_t item_slabid(uint8_t klen, uint32_t vlen)
{
    size_t ntotal;
    uint8_t id;

    ntotal = item_ntotal(klen, vlen, use_cas);

    id = slab_id(ntotal);
    if (id == SLABCLASS_INVALID_ID) {
        log_info("slab class id out of range with %"PRIu8" bytes "
                  "key, %"PRIu32" bytes value and %zu item chunk size", klen,
                  vlen, ntotal);
    }

    return id;
}

static void
_item_free(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);
    slab_put_item(it, it->id);
}

static void
_item_acquire_refcount(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    it->refcount++;
    slab_acquire_refcount(item_to_slab(it));
}

static void
_item_release_refcount(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(!(it->in_freeq));

    log_debug("remove it '%.*s' at offset %"PRIu32" with flags "
              "%d %d %d %d id %"PRId8" refcount %"PRIu16"",
              it->klen, item_key(it), it->offset, it->is_linked,
              it->has_cas, it->in_freeq, it->is_raligned, it->id,
              it->refcount);

    if (it->refcount != 0) {
        --it->refcount;
        slab_release_refcount(item_to_slab(it));
    }

    if (it->refcount == 0 && !(it->is_linked)) {
        _item_free(it);
    }
}

/*
 * Allocate an item. We allocate an item by consuming the next free item
 * from slab of the item's slab class.
 *
 * On success we return the pointer to the allocated item. The returned item
 * is refcounted so that it is not deleted under callers nose. It is the
 * callers responsibilty to release this refcount when the item is inserted
 * into the hash or is freed.
 */
struct item *
item_alloc(const struct bstring *key, rel_time_t exptime, uint32_t vlen)
{
    struct item *it;  /* item */
    uint8_t id = slab_id(item_ntotal(key->len, vlen, use_cas));

    ASSERT(id >= SLABCLASS_MIN_ID && id <= SLABCLASS_MAX_ID);

    it = slab_get_item(id);
    if (it != NULL) {
        /* 2) or 3) either we allow random eviction a free item is found */
        goto alloc_done;
    }

    log_warn("server error on allocating item in slab %"PRIu8, id);

    return NULL;

alloc_done:

    ASSERT(it->id == id);
    ASSERT(!(it->is_linked));
    ASSERT(!(it->in_freeq));
    ASSERT(it->offset != 0);
    ASSERT(it->refcount == 0);

    _item_acquire_refcount(it);

    it->is_linked = 0;
    it->has_cas = use_cas ? 1 : 0;
    it->in_freeq = 0;
    it->is_raligned = 0;
    it->vlen = vlen;
    it->exptime = exptime;
    it->klen = key->len;

/* #if defined MC_MEM_SCRUB && MC_MEM_SCRUB == 1 */
/*     memset(it->end, 0xff, slab_item_size(it->id) - ITEM_HDR_SIZE); */
/* #endif */

    cc_memcpy(item_key(it), key->data, key->len);
    item_set_cas(it, 0);

    log_verb("alloc it '%.*s' at offset %"PRIu32" with id %"PRIu8
             " expiry %u refcount %"PRIu16"", key->len, key->data,
             it->offset, it->id, exptime, it->refcount);

    return it;
}

/*
 * Make an item with zero refcount available for reuse by unlinking
 * it from the hash.
 *
 * Don't free the item yet because that would make it unavailable
 * for reuse.
 */
void
item_reuse(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(!(it->in_freeq));
    ASSERT(it->is_linked);
    ASSERT(it->refcount == 0);

    it->is_linked = 0;

    assoc_delete((uint8_t *)item_key(it), it->klen);

    log_verb("reuse %s it '%.*s' at offset %"PRIu32" with id "
              "%"PRIu8"", _item_expired(it) ? "expired" : "evicted",
              it->klen, item_key(it), it->offset, it->id);
}

/*
 * Link an item into the hash table
 */
static void
_item_link(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(!(it->is_linked));
    ASSERT(!(it->in_freeq));

    log_debug("link it '%.*s' at offset %"PRIu32" with flags "
              "%d %d %d %d id %"PRId8"", it->klen, item_key(it), it->offset,
              it->is_linked, it->has_cas, it->in_freeq, it->is_raligned, it->id);

    it->is_linked = 1;
    item_set_cas(it, _item_next_cas());

    assoc_insert(it);
}

/*
 * Unlinks an item from the hash table. Free an unlinked
 * item if it's refcount is zero.
 */
static void
_item_unlink(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);

    log_debug("unlink it '%.*s' at offset %"PRIu32" with flags "
              "%d %d %d %d id %"PRId8"", it->klen, item_key(it), it->offset,
              it->is_linked, it->has_cas, it->in_freeq, it->is_raligned, it->id);

    if (it->is_linked) {
        it->is_linked = 0;

        assoc_delete((uint8_t *)item_key(it), it->klen);

        if (it->refcount == 0) {
            _item_free(it);
        }
    }
}

/*
 * Replace one item with another in the hash table.
 */
static void
_item_relink(struct item *it, struct item *nit)
{
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(!(it->in_freeq));

    ASSERT(nit->magic == ITEM_MAGIC);
    ASSERT(!(nit->in_freeq));

    log_verb("relink it '%.*s' at offset %"PRIu32" id %"PRIu8" "
              "with one at offset %"PRIu32" id %"PRIu8"", it->klen,
              item_key(it), it->offset, it->id, nit->offset, nit->id);

    _item_unlink(it);
    _item_link(nit);
}

/*
 * Return an item if it hasn't been marked as expired, lazily expiring
 * item as-and-when needed
 *
 * When a non-null item is returned, it's the callers responsibily to
 * release refcount on the item
 */
struct item *
item_get(const struct bstring *key)
{
    struct item *it;

    it = assoc_find(key->data, key->len);
    if (it == NULL) {
        log_verb("get it '%.*s' not found", key->len, key->data);
        return NULL;
    }

    if (it->exptime != 0 && it->exptime <= time_now()) {
        _item_unlink(it);
        log_verb("get it '%.*s' expired and nuked", key->len, key->data);
        return NULL;
    }

    _item_acquire_refcount(it);

    log_verb("get it '%.*s' found at offset %"PRIu32" with flags "
             "%d %d %d %d id %"PRIu8" refcount %"PRIu32"", key->len, key->data,
             it->offset, it->is_linked, it->has_cas, it->in_freeq, it->is_raligned, it->id);

    return it;
}

void
item_set(const struct bstring *key, const struct bstring *val, rel_time_t exptime)
{
    struct item *it, *oit;

    it = item_alloc(key, exptime, val->len);
    cc_memcpy(item_data(it), val->data, val->len);

    oit = item_get(key);

    if (oit == NULL) {
        _item_link(it);
    } else {
        _item_relink(oit, it);
        _item_release_refcount(oit);
    }

    log_verb("store it '%.*s'at offset %"PRIu32" with flags %d %d %d %d"
             " id %"PRId8"", key->len, key->data, it->offset, it->is_linked,
             it->has_cas, it->in_freeq, it->is_raligned, it->id);

    _item_release_refcount(it);
}

rstatus_t
item_cas(const struct bstring *key, const struct bstring *val, rel_time_t exptime, uint64_t cas)
{
    rstatus_t ret;
    struct item *it = NULL, *oit;

    oit = item_get(key);

    if (oit == NULL) {
        ret = CC_ERROR;

        goto cas_done;
    }

    if (cas != item_get_cas(oit)) {
        log_debug("cas mismatch %"PRIu64" != %"PRIu64 "on "
                  "it '%.*s'", item_get_cas(oit), cas, key->len, key->data);

        ret = CC_ERROR;

        goto cas_done;
    }

    it = item_alloc(key, exptime, val->len);
    item_set_cas(it, cas);
    cc_memcpy(item_data(it), val->data, val->len);

    _item_relink(oit, it);
    ret = CC_OK;

    log_verb("cas it '%.*s'at offset %"PRIu32" with flags %d %d %d %d"
             " id %"PRId8"", key->len, key->data, it->offset, it->is_linked,
             it->has_cas, it->in_freeq, it->is_raligned, it->id);

cas_done:
    if (oit != NULL) {
        _item_release_refcount(oit);
    }

    if (it != NULL) {
        _item_release_refcount(it);
    }

    return ret;
}

rstatus_t
item_annex(const struct bstring *key, const struct bstring *val, bool append)
{
    rstatus_t ret;
    struct item *oit, *nit;
    uint8_t id;
    uint32_t total_nbyte;

    ret = CC_OK;

    oit = item_get(key);
    nit = NULL;
    if (oit == NULL) {
        ret = CC_ERROR;

        goto annex_done;
    }

    total_nbyte = oit->vlen + val->len;
    id = item_slabid(key->len, total_nbyte);
    if (id == SLABCLASS_INVALID_ID) {
        log_info("client error: annex operation results in oversized item"
                   "on key '%.*s' with key size %"PRIu8" and value size %"PRIu32,
                   key->len, key->data, key->len, total_nbyte);

        ret = CC_ERROR;

        goto annex_done;
    }

    log_verb("annex to oit '%.*s'at offset %"PRIu32" with flags %d %d %d %d"
             " id %"PRId8"", oit->klen, item_key(oit), oit->offset, oit->is_linked,
             oit->has_cas, oit->in_freeq, oit->is_raligned, oit->id);

    if (append) {
        /* if oit is large enough to hold the extra data and left-aligned,
         * which is the default behavior, we copy the delta to the end of
         * the existing data. Otherwise, allocate a new item and store the
         * payload left-aligned.
         */
        if (id == oit->id && !(oit->is_raligned)) {
            cc_memcpy(item_data(oit) + oit->vlen, val->data, val->len);
            oit->vlen = total_nbyte;
            item_set_cas(oit, _item_next_cas());
        } else {
            nit = item_alloc(key, oit->exptime, total_nbyte);
            if (nit == NULL) {
                ret = CC_ENOMEM;

                goto annex_done;
            }

            cc_memcpy(item_data(nit), item_data(oit), oit->vlen);
            cc_memcpy(item_data(nit) + oit->vlen, val->data, val->len);
            _item_relink(oit, nit);
        }
    } else {
        /* if oit is large enough to hold the extra data and is already
         * right-aligned, we copy the delta to the front of the existing
         * data. Otherwise, allocate a new item and store the payload
         * right-aligned, assuming more prepends will happen in the future.
         */
        if (id == oit->id && oit->is_raligned) {
            cc_memcpy(item_data(oit) - val->len, val->data, val->len);
            oit->vlen = total_nbyte;
            item_set_cas(oit, _item_next_cas());
        } else {
            nit = item_alloc(key, oit->exptime, total_nbyte);
            if (nit == NULL) {
                ret = CC_ENOMEM;

                goto annex_done;
            }

            nit->is_raligned = 1;
            cc_memcpy(item_data(nit) + val->len, item_data(oit), oit->vlen);
            cc_memcpy(item_data(nit), val->data, val->len);
            _item_relink(oit, nit);
        }
    }

    log_verb("annex successfully to it'%.*s', new id"PRId8,
             oit->klen, item_key(oit), id);


annex_done:
    if (oit != NULL) {
        _item_release_refcount(oit);
    }

    if (nit != NULL) {
        _item_release_refcount(nit);
    }

    return ret;
}

rstatus_t
item_update(struct item *it, const struct bstring *val)
{
    ASSERT(it != NULL);
    ASSERT(it->id != SLABCLASS_INVALID_ID);

    if(item_slabid(it->klen, val->len) != it->id) {
        /* val is oversized */
        return CC_ERROR;
    }

    it->vlen = val->len;
    cc_memcpy(item_data(it), val->data, val->len);

    return CC_OK;
}

rstatus_t
item_delete(const struct bstring *key)
{
    rstatus_t ret = CC_OK;
    struct item *it;

    it = item_get(key);
    if (it != NULL) {
        _item_unlink(it);
        _item_release_refcount(it);
    } else {
        ret = CC_ERROR;
    }

    return ret;
}
