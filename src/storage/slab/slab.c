#include <storage/slab/slab.h>
#include <storage/slab/item.h>

#include <cc_log.h>
#include <cc_mm.h>

#include <errno.h>
#include <stdlib.h>
#include <string.h>

#define SLAB_MODULE_NAME "storage::slab::slab"

struct slab_heapinfo {
    uint8_t         *base;       /* prealloc base */
    uint8_t         *curr;       /* prealloc start */
    uint32_t        nslab;       /* # slab allocated */
    uint32_t        max_nslab;   /* max # slab allowed */
    struct slab     **slab_table;/* table of all slabs */
    struct slab_tqh slab_lruq;   /* lru slab q */
};

struct slabclass slabclass[SLABCLASS_MAX_IDS];  /* collection of slabs bucketed by slabclass */
static struct slab_heapinfo heapinfo;           /* info of all allocated slabs */

size_t slab_size_setting;                    /* size of each slab */
bool use_cas;                                /* cas enabled? */

static bool prealloc;                        /* allocate slabs ahead of time? */
static int evict_opt;                        /* slab eviction policy */
static bool use_freeq;                       /* use items in free queue? */
static size_t chunk_size;                    /* chunk size */
static size_t maxbytes;                      /* maximum bytes allocated for slabs */
static size_t profile[SLABCLASS_MAX_IDS];    /* slab profile */
static uint8_t profile_last_id;              /* last id in slab profile */

static bool slab_init = false;
static slab_metrics_st *slab_metrics = NULL;

#define SLAB_RAND_MAX_TRIES         50
#define SLAB_LRU_MAX_TRIES          50

/*
 * Return the usable space for item sized chunks that would be carved out
 * of a given slab.
 */
size_t
slab_size(void)
{
    return slab_size_setting - SLAB_HDR_SIZE;
}

void
slab_print(void)
{
    uint8_t id;
    struct slabclass *p;

    loga("slab size %zu, slab hdr size %zu, item hdr size %zu, "
         "item chunk size %zu, total memory %zu", slab_size_setting,
         SLAB_HDR_SIZE, ITEM_HDR_SIZE, chunk_size, maxbytes);

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        p = &slabclass[id];

        loga("class %3"PRId8": items %7"PRIu32"  size %7zu  data %7zu  "
             "slack %7zu", id, p->nitem, p->size, p->size - ITEM_HDR_SIZE,
             slab_size() - p->nitem * p->size);
    }
}

void
slab_acquire_refcount(struct slab *slab)
{
    ASSERT(slab->magic == SLAB_MAGIC);
    slab->refcount++;
}

void
slab_release_refcount(struct slab *slab)
{
    ASSERT(slab->magic == SLAB_MAGIC);
    ASSERT(slab->refcount > 0);
    slab->refcount--;
}

/*
 * Get the idx^th item with a given size from the slab.
 */
static struct item *
_slab_to_item(struct slab *slab, uint32_t idx, size_t size)
{
    struct item *it;
    uint32_t offset = idx * size;

    ASSERT(slab->magic == SLAB_MAGIC);
    ASSERT(offset < slab_size_setting);

    it = (struct item *)((uint8_t *)slab->data + offset);

    return it;
}

/*
 * Return the item size given a slab id
 */
size_t
slab_item_size(uint8_t id) {
    ASSERT(id >= SLABCLASS_MIN_ID && id <= profile_last_id);

    return slabclass[id].size;
}

/*
 * Return the id of the slab which can store an item of a given size.
 *
 * Return SLABCLASS_INVALID_ID, for large items which cannot be stored in
 * any of the configured slabs.
 */
uint8_t
slab_id(size_t size)
{
    uint8_t id, imin, imax;

    ASSERT(size != 0);

    /* binary search */
    imin = SLABCLASS_MIN_ID;
    imax = profile_last_id;
    while (imax >= imin) {
        id = (imin + imax) / 2;
        if (size > slabclass[id].size) {
            imin = id + 1;
        } else if (id > SLABCLASS_MIN_ID && size <= slabclass[id - 1].size) {
            imax = id - 1;
        } else {
            break;
        }
    }

    if (imin > imax) {
        /* size too big for any slab */
        log_debug("slab_id: returning invalid");
        return SLABCLASS_INVALID_ID;
    }

    log_vverb("slab_id: returning %u", id);

    return id;
}

/*
 * Initialize all slabclasses.
 *
 * Every slabclass is a collection of slabs of fixed size specified by
 * --slab-size. A single slab is a collection of contiguous, equal sized
 * item chunks of a given size specified by the profile array
 */
static void
_slab_slabclass_setup(void)
{
    uint8_t id;      /* slabclass id */

    ASSERT(profile_last_id <= SLABCLASS_MAX_ID);

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        struct slabclass *p; /* slabclass */
        uint32_t nitem;      /* # item per slabclass */
        size_t item_sz;      /* item size */

        nitem = slab_size() / profile[id];
        item_sz = profile[id];
        p = &slabclass[id];

        p->nitem = nitem;
        p->size = item_sz;

        p->nfree_itemq = 0;
        SLIST_INIT(&p->free_itemq);

        p->nfree_item = 0;
        p->next_item_in_slab = NULL;
    }
}

static void
_slab_slabclass_teardown(void)
{
}

/*
 * Initialize slab heap related info
 *
 * When prelloc is true, the slab allocator allocates the entire heap
 * upfront. Otherwise, memory for new slabsare allocated on demand. But once
 * a slab is allocated, it is never freed, though a slab could be
 * reused on eviction.
 */
static rstatus_t
_slab_heapinfo_setup(void)
{
    heapinfo.nslab = 0;
    heapinfo.max_nslab = maxbytes / slab_size_setting;

    heapinfo.base = NULL;
    if (prealloc) {
        heapinfo.base = cc_alloc(heapinfo.max_nslab * slab_size_setting);
        if (heapinfo.base == NULL) {
            log_error("pre-alloc %zu bytes for %"PRIu32" slabs failed: %s",
                      heapinfo.max_nslab * slab_size_setting,
                      heapinfo.max_nslab, strerror(errno));
            return CC_ENOMEM;
        }

        log_info("pre-allocated %zu bytes for %"PRIu32" slabs",
                  maxbytes, heapinfo.max_nslab);
    }
    heapinfo.curr = heapinfo.base;

    heapinfo.slab_table = cc_alloc(sizeof(*heapinfo.slab_table) * heapinfo.max_nslab);
    if (heapinfo.slab_table == NULL) {
        log_error("create of slab table with %"PRIu32" entries failed: %s",
                  heapinfo.max_nslab, strerror(errno));
        return CC_ENOMEM;
    }
    TAILQ_INIT(&heapinfo.slab_lruq);

    log_vverb("created slab table with %"PRIu32" entries",
              heapinfo.max_nslab);

    INCR_N(slab_metrics, slab_heap_size, heapinfo.max_nslab);

    return CC_OK;
}

static void
_slab_heapinfo_teardown(void)
{
}

static rstatus_t
_slab_profile_setup(char *setup_profile, uint8_t setup_profile_last_id)
{
    int i;
    char *profile_entry;

    for(i = SLABCLASS_MIN_ID; i <= setup_profile_last_id; ++i) {
        profile_entry = strtok(setup_profile, " \n\r\t");

        if(profile_entry == NULL) {
            log_error("slab profile/profile_last_id mismatch - there are either "
                      "not enough profile entries or profile_last_id is too big");
            return CC_ERROR;
        }

        profile[i] = atol(profile_entry);
    }

    log_verb("setup slab profile setup_profile_last_id: %u", setup_profile_last_id);

    profile_last_id = setup_profile_last_id;

    return CC_OK;
}

/*
 * Initialize the slab module
 */
rstatus_t
slab_setup(size_t setup_slab_size, bool setup_use_cas, bool setup_prealloc, int setup_evict_opt,
           bool setup_use_freeq, size_t setup_chunk_size, size_t setup_maxbytes, char *setup_profile,
           uint8_t setup_profile_last_id, slab_metrics_st *metrics, uint32_t it_hash_power,
           item_metrics_st *it_metrics)
{
    rstatus_t ret;

    log_info("set up the %s module", SLAB_MODULE_NAME);

    if (slab_init) {
        log_warn("%s has already been set up, overwrite", SLAB_MODULE_NAME);
    }

    slab_size_setting = setup_slab_size;
    use_cas = setup_use_cas;
    prealloc = setup_prealloc;
    evict_opt = setup_evict_opt;
    use_freeq = setup_use_freeq;
    chunk_size = setup_chunk_size;
    maxbytes = setup_maxbytes;

    slab_metrics = metrics;
    SLAB_METRIC_INIT(slab_metrics);

    slab_init = true;

    ret = _slab_profile_setup(setup_profile, setup_profile_last_id);

    if (ret != CC_OK) {
        return ret;
    }

    _slab_slabclass_setup();

    ret = _slab_heapinfo_setup();

    if (ret != CC_OK) {
        return ret;
    }

    return item_setup(it_hash_power, it_metrics);
}

void
slab_teardown(void)
{
    log_info("tear down the %s module", SLAB_MODULE_NAME);

    if (!slab_init) {
        log_warn("%s has never been set up", SLAB_MODULE_NAME);
    }

    slab_metrics = NULL;
    _slab_heapinfo_teardown();
    _slab_slabclass_teardown();
    slab_init = false;
}

static void
_slab_hdr_init(struct slab *slab, uint8_t id)
{
    ASSERT(id >= SLABCLASS_MIN_ID && id <= profile_last_id);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    slab->magic = SLAB_MAGIC;
#endif
    slab->id = id;
    slab->unused = 0;
    slab->refcount = 0;
}

static bool
_slab_heap_full(void)
{
    return (heapinfo.nslab >= heapinfo.max_nslab);
}

static struct slab *
_slab_heap_create(void)
{
    struct slab *slab;

    if (prealloc) {
        slab = (struct slab *)heapinfo.curr;
        heapinfo.curr += slab_size_setting;
    } else {
        slab = cc_alloc(slab_size_setting);
        if (slab != NULL) {
            INCR(slab_metrics, slab_heap_size);
        }
    }

    return slab;
}

static void
_slab_table_update(struct slab *slab)
{
    ASSERT(heapinfo.nslab < heapinfo.max_nslab);

    heapinfo.slab_table[heapinfo.nslab] = slab;
    heapinfo.nslab++;

    log_verb("new slab %p allocated at pos %u", slab,
              heapinfo.nslab - 1);
}

static struct slab *
_slab_table_rand(void)
{
    uint32_t rand_idx;

    rand_idx = (uint32_t)rand() % heapinfo.nslab;
    return heapinfo.slab_table[rand_idx];
}

static struct slab *
_slab_lruq_head(void)
{
    return TAILQ_FIRST(&heapinfo.slab_lruq);
}

static void
_slab_lruq_append(struct slab *slab)
{
    log_vverb("append slab %p with id %d from lruq", slab, slab->id);
    TAILQ_INSERT_TAIL(&heapinfo.slab_lruq, slab, s_tqe);
}

static void
_slab_lruq_remove(struct slab *slab)
{
    log_vverb("remove slab %p with id %d from lruq", slab, slab->id);
    TAILQ_REMOVE(&heapinfo.slab_lruq, slab, s_tqe);
}

/*
 * Get a raw slab from the slab pool.
 */
static struct slab *
_slab_get_new(void)
{
    struct slab *slab;

    if (_slab_heap_full()) {
        return NULL;
    }

    slab = _slab_heap_create();
    if (slab == NULL) {
        return NULL;
    }

    _slab_table_update(slab);

    return slab;
}

/*
 * Evict a slab by evicting all the items within it. This means that the
 * items that are carved out of the slab must either be deleted from their
 * a) hash + lru Q, or b) free Q. The candidate slab itself must also be
 * delinked from its respective slab pool so that it is available for reuse.
 *
 * Eviction complexity is O(#items/slab).
 */
static void
_slab_evict_one(struct slab *slab)
{
    struct slabclass *p;
    struct item *it;
    uint32_t i;

    p = &slabclass[slab->id];

    /* candidate slab is also the current slab */
    if (p->next_item_in_slab != NULL && slab == item_to_slab(p->next_item_in_slab)) {
        p->nfree_item = 0;
        p->next_item_in_slab = NULL;
    }

    /* delete slab items either from hash or free Q */
    for (i = 0; i < p->nitem; i++) {
        it = _slab_to_item(slab, i, p->size);

        if (it->is_linked) {
            item_reuse(it);
        } else if (it->in_freeq) {
            ASSERT(slab == item_to_slab(it));
            ASSERT(!SLIST_EMPTY(&p->free_itemq));

            it->in_freeq = 0;

            ASSERT(p->nfree_itemq > 0);
            p->nfree_itemq--;
            SLIST_REMOVE(&p->free_itemq, it, item, i_sle);
        }
    }

    /* unlink the slab from its class */
    _slab_lruq_remove(slab);

    INCR(slab_metrics, slab_evict);
    DECR(slab_metrics, slab_curr);
}

/*
 * Get a random slab from all active slabs and evict it for new allocation.
 *
 * Note that the slab_table enables us to have O(1) lookup for every slab in
 * the system. The inserts into the table are just appends - O(1) and there
 * are no deletes from the slab_table. These two constraints allows us to keep
 * our random choice uniform.
 */
static struct slab *
_slab_evict_rand(void)
{
    struct slab *slab;
    uint32_t tries;

    tries = SLAB_RAND_MAX_TRIES;
    do {
        slab = _slab_table_rand();
        tries--;
    } while (tries > 0 && slab->refcount != 0);

    if (tries == 0) {
        /* all randomly chosen slabs are in use */
        return NULL;
    }

    log_debug("random-evicting slab %p with id %u", slab, slab->id);

    _slab_evict_one(slab);

    return slab;
}

/*
 * Evict by looking into least recently used queue of all slabs.
 */
static struct slab *
_slab_evict_lru(int id)
{
    struct slab *slab;
    uint32_t tries;

    for (tries = SLAB_LRU_MAX_TRIES, slab = _slab_lruq_head();
         tries > 0 && slab != NULL;
         tries--, slab = TAILQ_NEXT(slab, s_tqe)) {
        if (slab->refcount == 0) {
            break;
        }
    }

    if (tries == 0 || slab == NULL) {
        return NULL;
    }

    log_debug("lru-evicting slab %p with id %u", slab, slab->id);

    _slab_evict_one(slab);

    return slab;
}

/*
 * All the prep work before start using a slab.
 */
static void
_slab_init(struct slab *slab, uint8_t id)
{
    struct slabclass *p;
    struct item *it;
    uint32_t i, offset;

    p = &slabclass[id];

    /* initialize slab header */
    _slab_hdr_init(slab, id);

    _slab_lruq_append(slab);

    /* initialize all slab items */
    for (i = 0; i < p->nitem; i++) {
        it = _slab_to_item(slab, i, p->size);
        offset = (uint32_t)((uint8_t *)it - (uint8_t *)slab);
        item_hdr_init(it, offset, id);
    }

    /* make this slab as the current slab */
    p->nfree_item = p->nitem;
    p->next_item_in_slab = (struct item *)&slab->data[0];

    INCR(slab_metrics, slab_curr);
}

/*
 * Get a slab.
 *   id is the slabclass the new slab will be linked into.
 *
 * We return a slab either from the:
 * 1. slab pool, if not empty. or,
 * 2. evict an active slab and return that instead.
 */
static rstatus_t
_slab_get(uint8_t id)
{
    rstatus_t status;
    struct slab *slab;

    ASSERT(slabclass[id].next_item_in_slab == NULL);
    ASSERT(SLIST_EMPTY(&slabclass[id].free_itemq));

    slab = _slab_get_new();

    if (slab == NULL && (evict_opt & EVICT_CS)) {
        slab = _slab_evict_lru(id);
    }

    if (slab == NULL && (evict_opt & EVICT_RS)) {
        slab = _slab_evict_rand();
    }

    if (slab != NULL) {
        _slab_init(slab, id);
        status = CC_OK;
    } else {
        status = CC_ENOMEM;
        INCR(slab_metrics, slab_req_ex);
    }

    INCR(slab_metrics, slab_req);

    return status;
}

/*
 * Get an item from the item free q of the given slab with id.
 */
static struct item *
_slab_get_item_from_freeq(uint8_t id)
{
    struct slabclass *p; /* parent slabclass */
    struct item *it;

    if (!use_freeq) {
        return NULL;
    }

    p = &slabclass[id];

    if (p->nfree_itemq == 0) {
        return NULL;
    }

    it = SLIST_FIRST(&p->free_itemq);

    ASSERT(it->magic == ITEM_MAGIC);
    ASSERT(it->in_freeq);
    ASSERT(!(it->is_linked));

    it->in_freeq = 0;

    ASSERT(p->nfree_itemq > 0);
    p->nfree_itemq--;
    SLIST_REMOVE(&p->free_itemq, it, item, i_sle);

    log_verb("get free q it '%.*s' at offset %"PRIu32" with id "
             "%"PRIu8"", it->klen, item_key(it), it->offset, it->id);

    return it;
}

/*
 * Get an item from the slab with a given id. We get an item either from:
 * 1. item free Q of given slab with id. or,
 * 2. current slab.
 * If the current slab is empty, we get a new slab from the slab allocator
 * and return the next item from this new slab.
 */
static struct item *
_slab_get_item(uint8_t id)
{
    struct slabclass *p;
    struct item *it;

    p = &slabclass[id];

    it = _slab_get_item_from_freeq(id);
    if (it != NULL) {
        return NULL;
    }

    if (p->next_item_in_slab == NULL && (_slab_get(id) != CC_OK)) {
        return NULL;
    }

    /* return item from current slab */
    it = p->next_item_in_slab;
    if (--p->nfree_item != 0) {
        p->next_item_in_slab = (struct item *)(((uint8_t *)p->next_item_in_slab) + p->size);
    } else {
        p->next_item_in_slab = NULL;
    }

    log_verb("get new it at offset %"PRIu32" with id %"PRIu8"",
              it->offset, it->id);

    return it;
}

struct item *
slab_get_item(uint8_t id)
{
    struct item *it;

    ASSERT(id >= SLABCLASS_MIN_ID && id <= profile_last_id);

    it = _slab_get_item(id);

    return it;
}

/*
 * Put an item back into the slab by inserting into the item free Q.
 */
static void
_slab_put_item_into_freeq(struct item *it, uint8_t id)
{
    struct slabclass *p = &slabclass[id];

    ASSERT(id >= SLABCLASS_MIN_ID && id <= profile_last_id);
    ASSERT(item_to_slab(it)->id == id);
    ASSERT(!(it->is_linked));
    ASSERT(!(it->in_freeq));
    ASSERT(it->refcount == 0);
    ASSERT(it->offset != 0);

    log_verb("put free q it '%.*s' at offset %"PRIu32" with id "
              "%"PRIu8"", it->klen, item_key(it), it->offset, it->id);

    it->in_freeq = 0;

    p->nfree_itemq++;
    SLIST_INSERT_HEAD(&p->free_itemq, it, i_sle);
}

/*
 * Put an item back into the slab
 */
void
slab_put_item(struct item *it, uint8_t id)
{
    _slab_put_item_into_freeq(it, id);
}
