#include "slab.h"

#include "hashtable.h"
#include "item.h"

#include <cc_mm.h>
#include <cc_util.h>

#include <errno.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

#define SLAB_MODULE_NAME        "storage::slab"
#define SLAB_ALIGN_DOWN(d, n)   ((d) - ((d) % (n)))
#define TRIES_MAX               10

struct slab_heapinfo {
    uint8_t         *base;       /* prealloc base */
    uint8_t         *curr;       /* prealloc start */
    uint32_t        nslab;       /* # slab allocated */
    uint32_t        max_nslab;   /* max # slab allowed */
    struct slab     **slab_table;/* table of all slabs */
    struct slab_tqh slab_lruq;   /* lru slab q */
};

perslab_metrics_st perslab[SLABCLASS_MAX_ID];
uint8_t profile_last_id; /* last id in slab profile */

static struct slab_heapinfo heapinfo;             /* info of all allocated slabs */
static size_t profile[SLABCLASS_MAX_ID + 1];      /* slab profile */
struct slabclass slabclass[SLABCLASS_MAX_ID + 1]; /* collection of slabs bucketed by slabclass */

size_t slab_size = SLAB_SIZE;           /* # bytes in a slab */
static size_t slab_mem = SLAB_MEM;      /* maximum bytes allocated for slabs */
static bool prealloc = SLAB_PREALLOC;   /* allocate slabs ahead of time? */
static int evict_opt = SLAB_EVICT_OPT;  /* slab eviction policy */
static bool use_freeq = SLAB_USE_FREEQ; /* use items in free queue? */
static size_t item_min = ITEM_SIZE_MIN; /* min item size */
static size_t item_max = ITEM_SIZE_MAX; /* max item size */
static double item_growth = ITEM_FACTOR;/* item size growth factor */
static uint32_t hash_power = HASH_POWER;/* power (of 2) entries for hashtable */

bool use_cas = SLAB_USE_CAS;
struct hash_table *hash_table = NULL;
uint64_t cas_id;

static bool slab_init = false;
slab_metrics_st *slab_metrics = NULL;

void
slab_print(void)
{
    uint8_t id;
    struct slabclass *p;

    loga("slab size %zu, slab hdr size %zu, item hdr size %zu, item chunk size"
            "%zu, total memory %zu", slab_size, SLAB_HDR_SIZE, ITEM_HDR_SIZE,
            item_min, slab_mem);

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        p = &slabclass[id];

        loga("class %3"PRId8": items %7"PRIu32"  size %7zu  data %7zu  "
             "slack %7zu", id, p->nitem, p->size, p->size - ITEM_HDR_SIZE,
             slab_capacity() - p->nitem * p->size);
    }
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
    ASSERT(offset < slab_size);

    it = (struct item *)((char *)slab->data + offset);

    return it;
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
static rstatus_i
_slab_slabclass_setup(void)
{
    uint8_t id;      /* slabclass id */

    ASSERT(profile_last_id <= SLABCLASS_MAX_ID);

    for (id = SLABCLASS_MIN_ID; id <= profile_last_id; id++) {
        struct slabclass *p; /* slabclass */
        uint32_t nitem;      /* # item per slabclass */
        size_t item_sz;      /* item size */

        nitem = slab_capacity() / profile[id];

        if (nitem == 0) {
            log_error("Invalid slab class size %u; too large to fit in slab!",
                    profile[id]);
            return CC_ERROR;
        }

        item_sz = profile[id];
        p = &slabclass[id];

        p->nitem = nitem;
        p->size = item_sz;

        /* chunk_size is static */
        perslab[id] = (perslab_metrics_st){PERSLAB_METRIC(METRIC_INIT)};
        UPDATE_VAL(&perslab[id], chunk_size, item_sz);

        p->nfree_itemq = 0;
        SLIST_INIT(&p->free_itemq);

        p->nfree_item = 0;
        p->next_item_in_slab = NULL;
    }

    return CC_OK;
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
static rstatus_i
_slab_heapinfo_setup(void)
{
    heapinfo.nslab = 0;
    heapinfo.max_nslab = slab_mem / slab_size;

    heapinfo.base = NULL;
    if (prealloc) {
        heapinfo.base = cc_alloc(heapinfo.max_nslab * slab_size);
        if (heapinfo.base == NULL) {
            log_error("pre-alloc %zu bytes for %"PRIu32" slabs failed: %s",
                      heapinfo.max_nslab * slab_size, heapinfo.max_nslab,
                      strerror(errno));
            return CC_ENOMEM;
        }

        log_info("pre-allocated %zu bytes for %"PRIu32" slabs",
                  slab_mem, heapinfo.max_nslab);
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

    return CC_OK;
}

static void
_slab_heapinfo_teardown(void)
{
}

static rstatus_i
_slab_profile_setup(char *profile_str)
{
    int i;

    /* TODO(yao): check alignment (with machine word length) in the user config,
     * reject ones that don't align
     */
    if (profile_str != NULL) {
        /* slab profile specified */
        char *profile_entry;

        i = SLABCLASS_MIN_ID - 1;

        do {
            /*TODO(yao): strsep alters the original slab profile string, which
             * is probably not desirable (we don't want to modify options once
             * they are loaded. Do another memcpy to some local variable.
             */
            profile_entry = strsep(&profile_str, " \n\r\t");
            profile[++i] = atol(profile_entry);
            if (profile[i] <= profile[i - 1]) {
                log_error("Invalid setup profile configuration provided");
                return CC_ERROR;
            }
        } while (profile_str != NULL);

        profile_last_id = i;
    } else {
        /* generate slab profile using chunk size, slab size, and factor */
        size_t nbyte, nitem, linear_nitem;

        if (item_min <= ITEM_HDR_SIZE) {
            log_error("invalid min chunk size - too small for item overhead");
            return CC_ERROR;
        }

        if (item_max + SLAB_HDR_SIZE > slab_size) {
            log_error("invalid max chunk size - too large to fit in one slab");
            return CC_ERROR;
        }

        if (item_min > item_max) {
            log_error("Could not setup slab profile; invalid min/max chunk size");
            return CC_ERROR;
        }

        if (item_growth <= 1.0) {
            log_error("Could not setup slab profile; invalid growth factor");
            return CC_ERROR;
        }

        /*
         * Slab profile is determined as follows:
         *
         * Exponential growth of item size based on gf^n, from which we determine
         * the # items that can fit in the slab. At the point that the # items
         * change is <= 1, we form each slab profile entry as 1 item less than
         * the last until we hit 1 item.
         *
         * Example (assuming max chunk size == slab size):
         *
         * Suppose gf == 1.2, so we determine 1.2 * x >= x + 1 --> x >= 5
         * Thus, once we hit nitem == 5, we reduce nitem linearly by 1 until we
         * reach nitem == 1:
         *
         * Exponential growth while nitem increase still > 1
         * +-----------------------------------------------------------+
         * |   |   |   |   |   |   |   |   |   |   |   |   |   |   |   |
         * +-----------------------------------------------------------+
         *
         * +-----------------------------------------------------------+
         * |    |    |    |    |    |    |    |    |    |    |    |    |
         * +-----------------------------------------------------------+
         *
         * +-----------------------------------------------------------+
         * |     |     |     |     |     |     |     |     |     |     |
         * +-----------------------------------------------------------+
         *
         *
         *                               .
         *                               .
         *                               .
         *
         *
         * Transition to linear growth
         * +-----------------------------------------------------------+
         * |              |              |              |              |
         * +-----------------------------------------------------------+
         *
         * +-----------------------------------------------------------+
         * |                   |                   |                   |
         * +-----------------------------------------------------------+
         *
         * +-----------------------------------------------------------+
         * |                             |                             |
         * +-----------------------------------------------------------+
         *
         * +-----------------------------------------------------------+
         * |                                                           |
         * +-----------------------------------------------------------+
         */

        linear_nitem = 1.0 / (item_growth - 1.0);
        i = SLABCLASS_MIN_ID;
        nitem = slab_capacity() / (CC_ALIGN(item_min, CC_ALIGNMENT));
        nbyte = slab_capacity() / nitem;

        /* exponential growth phase */
        while (nbyte <= item_max && nitem > linear_nitem) {
            if (i > SLABCLASS_MAX_ID) {
                log_error("Slab profile improperly configured - max chunk size "
                          "too large or growth factor too small");
                return CC_ERROR;
            }

            if (profile[i - 1] == nbyte) {
                nbyte += CC_ALIGNMENT;
            }

            profile[i++] = nbyte;
            nitem = slab_capacity() / nbyte / item_growth;
            nbyte = SLAB_ALIGN_DOWN(slab_capacity() / nitem, CC_ALIGNMENT);
        }

        /* linear growth phase */
        nitem = linear_nitem;
        nbyte = SLAB_ALIGN_DOWN(slab_capacity() / nitem, CC_ALIGNMENT);
        while (nbyte <= item_max && nitem > 0) {
            if (i > SLABCLASS_MAX_ID) {
                log_error("Slab profile improperly configured - max chunk size "
                          "too large or growth factor too small");
                return CC_ERROR;
            }
            profile[i++] = nbyte;
            if (--nitem > 0) {
                nbyte = SLAB_ALIGN_DOWN(slab_capacity() / nitem, CC_ALIGNMENT);
            }
        }

        profile_last_id = i - 1;
    }

    log_verb("setup slab profile profile_last_id: %u", profile_last_id);
    log_verb("slab profile:");

    for (i = SLABCLASS_MIN_ID; i <= profile_last_id; ++i) {
        log_verb("%u", profile[i]);
    }

    return CC_OK;
}

void
slab_teardown(void)
{
    log_info("tear down the %s module", SLAB_MODULE_NAME);

    if (!slab_init) {
        log_warn("%s has never been set up", SLAB_MODULE_NAME);
    }

    hashtable_destroy(hash_table);
    _slab_heapinfo_teardown();
    _slab_slabclass_teardown();
    slab_metrics = NULL;

    slab_init = false;
}

void
slab_setup(slab_options_st *options, slab_metrics_st *metrics)
{
    char *profile_str = SLAB_PROFILE;

    log_info("set up the %s module", SLAB_MODULE_NAME);

    if (slab_init) {
        log_warn("%s has already been set up, re-creating", SLAB_MODULE_NAME);
        slab_teardown();
    }

    log_verb("Slab header size: %d, item header size: %d", SLAB_HDR_SIZE,
            ITEM_HDR_SIZE);

    slab_metrics = metrics;

    if (options != NULL) {
        slab_size = option_uint(&options->slab_size);
        slab_mem = option_uint(&options->slab_mem);
        prealloc = option_bool(&options->slab_prealloc);
        evict_opt = option_uint(&options->slab_evict_opt);
        use_freeq = option_bool(&options->slab_use_freeq);
        profile_str = option_str(&options->slab_profile);
        item_min = option_uint(&options->slab_item_min);
        item_max = option_uint(&options->slab_item_max);
        item_growth = option_fpn(&options->slab_item_growth);
        use_cas = option_bool(&options->slab_use_cas);
        hash_power = option_uint(&options->slab_hash_power);
    }

    hash_table = hashtable_create(hash_power);
    if (hash_table == NULL) {
        log_crit("Could not create hash table");
        goto error;
    }

    if (_slab_heapinfo_setup() != CC_OK) {
        log_crit("Could not setup slab heap info");
        goto error;
    }

    if (_slab_profile_setup(profile_str) != CC_OK) {
        log_crit("Could not setup slab profile");
        goto error;
    }

    if (_slab_slabclass_setup() != CC_OK) {
        log_crit("Could not setup slabclasses");
        goto error;
    }

    slab_init = true;

    return;

error:
    slab_teardown();
    exit(EX_CONFIG);
}

static void
_slab_hdr_init(struct slab *slab, uint8_t id)
{
    ASSERT(id >= SLABCLASS_MIN_ID && id <= profile_last_id);

#if defined CC_ASSERT_PANIC || defined CC_ASSERT_LOG
    slab->magic = SLAB_MAGIC;
#endif
    slab->id = id;
    slab->padding = 0;
    slab->refcount = 0;
}

static bool
_slab_heap_full(void)
{
    return heapinfo.nslab >= heapinfo.max_nslab;
}

static struct slab *
_slab_heap_create(void)
{
    struct slab *slab;

    if (prealloc) {
        slab = (struct slab *)heapinfo.curr;
        heapinfo.curr += slab_size;
    } else {
        slab = cc_alloc(slab_size);
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
    INCR(slab_metrics, slab_curr);
    PERSLAB_INCR(slab->id, slab_curr);
    INCR_N(slab_metrics, slab_memory, slab_size);

    return slab;
}


/*
 * Because a slab can have a reserved item (claimed but not linked), which is
 * requested when a write command does not have the entirety of value in the
 * buffer, eviction will fail if the slab has a non-zero refcount. True is
 * returned if the slab is successfully evicted, False if eviction is denied.
 */
static inline bool
_slab_evict_ok(struct slab *slab)
{
    return (slab->refcount == 0);
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

    INCR(slab_metrics, slab_evict);

    /* candidate slab is also the current slab */
    if (p->next_item_in_slab != NULL && slab == item_to_slab(p->next_item_in_slab)) {
        p->nfree_item = 0;
        p->next_item_in_slab = NULL;
    }

    /* delete slab items either from hash or free Q */
    for (i = 0; i < p->nitem; i++) {
        it = _slab_to_item(slab, i, p->size);

        if (it->is_linked) {
            it->is_linked = 0;
            hashtable_delete(item_key(it), it->klen, hash_table);
        } else if (it->in_freeq) {
            ASSERT(slab == item_to_slab(it));
            ASSERT(!SLIST_EMPTY(&p->free_itemq));
            ASSERT(p->nfree_itemq > 0);

            it->in_freeq = 0;
            p->nfree_itemq--;
            SLIST_REMOVE(&p->free_itemq, it, item, i_sle);
        }
    }

    /* unlink the slab from its class */
    _slab_lruq_remove(slab);
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
    int i = 0;

    do {
        slab = _slab_table_rand();
    } while (slab != NULL && ++i < TRIES_MAX && !_slab_evict_ok(slab));

    if (slab == NULL) {
        /* warning here because eviction failure should be rare. This can
         * indicate there are dead/idle connections hanging onto items and
         * slab refcounts.
         */
        log_warn("can't find a slab for random-evicting slab with %d tries", i);
    } else {
        log_verb("random-evicting slab %p with id %u", slab, slab->id);
        _slab_evict_one(slab);
    }

    return slab;
}

/*
 * Evict by looking into least recently used queue of all slabs.
 */
static struct slab *
_slab_evict_lru(int id)
{
    struct slab *slab = _slab_lruq_head();
    int i = 0;

    while (slab != NULL && ++i < TRIES_MAX && !_slab_evict_ok(slab)) {
        slab = TAILQ_NEXT(slab, s_tqe);
    };

    if (slab == NULL) {
        /* warning here because eviction failure should be rare. This can
         * indicate there are dead/idle connections hanging onto items and
         * slab refcounts.
         */
        log_warn("can't find a slab for lru-evicting slab with %d tries", i);
    } else {
        log_verb("lru-evicting slab %p with id %u", slab, slab->id);
        _slab_evict_one(slab);
    }

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
        offset = (uint32_t)((char *)it - (char *)slab);
        item_hdr_init(it, offset, id);
    }

    /* make this slab as the current slab */
    p->nfree_item = p->nitem;
    p->next_item_in_slab = (struct item *)&slab->data[0];
}

/*
 * Get a slab.
 *   id is the slabclass the new slab will be linked into.
 *
 * We return a slab either from the:
 * 1. slab pool, if not empty. or,
 * 2. evict an active slab and return that instead.
 */
static rstatus_i
_slab_get(uint8_t id)
{
    rstatus_i status;
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
    PERSLAB_DECR(id, item_free);

    log_verb("get free q it %p at offset %"PRIu32" with id %"PRIu8, it,
            it->offset, it->id);

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
        return it;
    }

    if (p->next_item_in_slab == NULL && (_slab_get(id) != CC_OK)) {
        return NULL;
    }

    /* return item from current slab */
    it = p->next_item_in_slab;
    if (--p->nfree_item != 0) {
        p->next_item_in_slab = (struct item *)((char *)p->next_item_in_slab + p->size);
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
    ASSERT(it->offset != 0);

    log_verb("put free q it %p at offset %"PRIu32" with id %"PRIu8, it,
            it->offset, it->id);

    it->in_freeq = 1;

    p->nfree_itemq++;
    SLIST_INSERT_HEAD(&p->free_itemq, it, i_sle);

    PERSLAB_INCR(id, item_free);
}

/*
 * Put an item back into the slab
 */
void
slab_put_item(struct item *it, uint8_t id)
{
    _slab_put_item_into_freeq(it, id);
}
