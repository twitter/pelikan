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

    if (item_is_raligned(it)) {
        data = (char *)it + slab_item_size(it->id) - it->nval;
    } else {
        data = it->end + it->nkey + (item_has_cas(it) ? sizeof(uint64_t) : 0);
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
    it->flags = 0;
}

uint8_t item_slabid(uint8_t nkey, uint32_t nval)
{
    size_t ntotal;
    uint8_t id;

    ntotal = item_ntotal(nkey, nval, use_cas);

    id = slab_id(ntotal);
    if (id == SLABCLASS_INVALID_ID) {
        log_info("slab class id out of range with %"PRIu8" bytes "
                  "key, %"PRIu32" bytes value and %zu item chunk size", nkey,
                  nval, ntotal);
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
    ASSERT(!item_is_slabbed(it));

    log_debug("remove it '%.*s' at offset %"PRIu32" with flags "
              "%02x id %"PRId8" refcount %"PRIu16"", it->nkey, item_key(it),
              it->offset, it->flags, it->id, it->refcount);

    if (it->refcount != 0) {
        --it->refcount;
        slab_release_refcount(item_to_slab(it));
    }

    if (it->refcount == 0 && !item_is_linked(it)) {
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
item_alloc(const struct bstring *key, rel_time_t exptime, uint32_t nval)
{
    struct item *it;  /* item */
    uint8_t id = slab_id(item_ntotal(key->len, nval, use_cas));

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
    ASSERT(!item_is_linked(it));
    ASSERT(!item_is_slabbed(it));
    ASSERT(it->offset != 0);
    ASSERT(it->refcount == 0);

    _item_acquire_refcount(it);

    it->flags = use_cas ? ITEM_CAS : 0;
    it->nval = nval;
    it->exptime = exptime;
    it->nkey = key->len;

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
    ASSERT(!item_is_slabbed(it));
    ASSERT(item_is_linked(it));
    ASSERT(it->refcount == 0);

    item_set_flag(it, ITEM_LINKED, false);

    assoc_delete((uint8_t *)item_key(it), it->nkey);

    log_verb("reuse %s it '%.*s' at offset %"PRIu32" with id "
              "%"PRIu8"", _item_expired(it) ? "expired" : "evicted",
              it->nkey, item_key(it), it->offset, it->id);
}

/*
 * Link an item into the hash table
 */
static void
_item_link(struct item *it)
{
    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(!item_is_linked(it));
    ASSERT(!item_is_slabbed(it));

    log_debug("link it '%.*s' at offset %"PRIu32" with flags "
              "%02x id %"PRId8"", it->nkey, item_key(it), it->offset,
              it->flags, it->id);

    item_set_flag(it, ITEM_LINKED, true);
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
              "%02x id %"PRId8"", it->nkey, item_key(it), it->offset,
              it->flags, it->id);

    if (item_is_linked(it)) {
        item_set_flag(it, ITEM_LINKED, false);

        assoc_delete((uint8_t *)item_key(it), it->nkey);

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
    ASSERT(!item_is_slabbed(it));

    ASSERT(nit->magic == ITEM_MAGIC);
    ASSERT(!item_is_slabbed(nit));

    log_verb("relink it '%.*s' at offset %"PRIu32" id %"PRIu8" "
              "with one at offset %"PRIu32" id %"PRIu8"", it->nkey,
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
             "%02x id %"PRIu8" refcount %"PRIu32"", key->len, key->data,
             it->offset, it->flags, it->id);

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

    log_verb("store it '%.*s'at offset %"PRIu32" with flags %02x"
              " id %"PRId8"", key->len, key->data, it->offset, it->flags,
              it->id);

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

    log_verb("cas it '%.*s'at offset %"PRIu32" with flags %02x"
             " id %"PRId8"", key->len, key->data, it->offset, it->flags,
             it->id);

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

    total_nbyte = oit->nval + val->len;
    id = item_slabid(key->len, total_nbyte);
    if (id == SLABCLASS_INVALID_ID) {
        log_info("client error: annex operation results in oversized item"
                   "on key '%.*s' with key size %"PRIu8" and value size %"PRIu32,
                   key->len, key->data, key->len, total_nbyte);

        ret = CC_ERROR;

        goto annex_done;
    }

    log_verb("annex to oit '%.*s'at offset %"PRIu32" with flags %02x"
              " id %"PRId8"", oit->nkey, item_key(oit), oit->offset, oit->flags,
              oit->id);

    if (append) {
        /* if oit is large enough to hold the extra data and left-aligned,
         * which is the default behavior, we copy the delta to the end of
         * the existing data. Otherwise, allocate a new item and store the
         * payload left-aligned.
         */
        if (id == oit->id && !item_is_raligned(oit)) {
            cc_memcpy(item_data(oit) + oit->nval, val->data, val->len);
            oit->nval = total_nbyte;
            item_set_cas(oit, _item_next_cas());
        } else {
            nit = item_alloc(key, oit->exptime, total_nbyte);
            if (nit == NULL) {
                ret = CC_ENOMEM;

                goto annex_done;
            }

            cc_memcpy(item_data(nit), item_data(oit), oit->nval);
            cc_memcpy(item_data(nit) + oit->nval, val->data, val->len);
            _item_relink(oit, nit);
        }
    } else {
        /* if oit is large enough to hold the extra data and is already
         * right-aligned, we copy the delta to the front of the existing
         * data. Otherwise, allocate a new item and store the payload
         * right-aligned, assuming more prepends will happen in the future.
         */
        if (id == oit->id && item_is_raligned(oit)) {
            cc_memcpy(item_data(oit) - val->len, val->data, val->len);
            oit->nval = total_nbyte;
            item_set_cas(oit, _item_next_cas());
        } else {
            nit = item_alloc(key, oit->exptime, total_nbyte);
            if (nit == NULL) {
                ret = CC_ENOMEM;

                goto annex_done;
            }

            item_set_flag(nit, ITEM_RALIGN, true);
            cc_memcpy(item_data(nit) + val->len, item_data(oit), oit->nval);
            cc_memcpy(item_data(nit), val->data, val->len);
            _item_relink(oit, nit);
        }
    }

    log_verb("annex successfully to it'%.*s', new id"PRId8,
             oit->nkey, item_key(oit), id);


annex_done:
    if (oit != NULL) {
        _item_release_refcount(oit);
    }

    if (nit != NULL) {
        _item_release_refcount(nit);
    }

    return ret;
}

/*
 * Apply a delta value (positive or negative) to an item.
 */
rstatus_t
item_delta(const struct bstring *key, int64_t delta, rel_time_t exptime)
{
    rstatus_t ret = CC_OK;
    struct item *it;

    it = item_get(key);
    if (it == NULL) {
        return CC_ERROR;

        goto delta_done;
    }

    if (!item_is_integer(it)) {
        ret = CC_ERROR;
        goto delta_done;
    }

    *((int64_t*)item_data(it)) += delta;
    item_set_cas(it, _item_next_cas());
    it->exptime = exptime;

delta_done:

    _item_release_refcount(it);

    return ret;
}

/*
 * Unlink an item and remove it (if its recount drops to zero).
 */
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
