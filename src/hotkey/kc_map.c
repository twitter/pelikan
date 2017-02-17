#include "kc_map.h"

#include "constant.h"

#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_hash.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define KC_MAP_MODULE_NAME "hotkey::kc_map"

struct kc_map_entry {
    STAILQ_ENTRY(kc_map_entry) next; /* entry in hash table or pool */

    char                       key[MAX_KEY_LEN];
    uint32_t                   klen;
    uint32_t                   count;
};

STAILQ_HEAD(kcme_slh, kc_map_entry);

static struct kcme_slh *table = NULL;
static uint32_t table_size = 0; /* number of buckets in table */
static bool kc_map_init = false;

FREEPOOL(kcme_pool, kcmeq, kc_map_entry);
static struct kcme_pool kcmep;
static bool kcmep_init = false;

static void
kc_map_entry_reset(struct kc_map_entry *kcme)
{
    kcme->klen = 0;
    kcme->count = 0;
}

static struct kc_map_entry *
kc_map_entry_create(void)
{
    struct kc_map_entry *kcme = cc_alloc(sizeof(*kcme));

    if (kcme == NULL) {
        return NULL;
    }

    kc_map_entry_reset(kcme);

    return kcme;
}

static void
kc_map_entry_destroy(struct kc_map_entry **kc_map_entry)
{
    struct kc_map_entry *kcme = *kc_map_entry;
    ASSERT(kcme != NULL);

    cc_free(kcme);
    *kc_map_entry = NULL;
}

static void
kc_map_entry_pool_destroy(void)
{
    struct kc_map_entry *kcme, *tkcme;

    if (!kcmep_init) {
        log_warn("kc_map_entry pool was not created, ignore");
        return;
    }

    log_info("destroying kc_map_entry pool: free %"PRIu32, kcmep.nfree);

    FREEPOOL_DESTROY(kcme, tkcme, &kcmep, next, kc_map_entry_destroy);
    kcmep_init = false;
}

static struct kc_map_entry *
kc_map_entry_borrow(void)
{
    struct kc_map_entry *kcme;

    FREEPOOL_BORROW(kcme, &kcmep, next, kc_map_entry_create);
    if (kcme == NULL) {
        log_debug("borrow kc_map_entry failed: OOM");
        return NULL;
    }
    kc_map_entry_reset(kcme);

    return kcme;
}

static void
kc_map_entry_return(struct kc_map_entry **kc_map_entry)
{
    struct kc_map_entry *kcme = *kc_map_entry;

    if (kcme == NULL) {
        return;
    }

    FREEPOOL_RETURN(kcme, &kcmep, next);

    *kc_map_entry = NULL;
}

static void
kc_map_entry_pool_create(uint32_t max)
{
    struct kc_map_entry *kcme;

    if (kcmep_init) {
        log_warn("kc_map_entry pool has already been created, re-creating");
        kc_map_entry_pool_destroy();
    }

    log_info("creating kc_map_entry pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&kcmep, max);
    kcmep_init = true;

    FREEPOOL_PREALLOC(kcme, &kcmep, max, next, kc_map_entry_create);
    if (kcmep.nfree < max) {
        log_crit("cannot preallocate kc_map_entry pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

void
kc_map_setup(uint32_t size, uint32_t poolsize)
{
    uint32_t i;

    log_info("Set up the %s module", KC_MAP_MODULE_NAME);

    if (kc_map_init) {
        log_warn("%s has already been setup, ignore", KC_MAP_MODULE_NAME);
        return;
    }

    table = cc_alloc(sizeof(*table) * size);
    table_size = size;

    if (table == NULL) {
        log_crit("Could not allocate counter table for hotkey - OOM");
        exit(EXIT_FAILURE);
    }

    for (i = 0; i < size; ++i) {
        STAILQ_INIT(&table[i]);
    }

    kc_map_entry_pool_create(poolsize);
}

void
kc_map_teardown(void)
{
    log_info("Tear down the %s module", KC_MAP_MODULE_NAME);

    if (!kc_map_init) {
        log_warn("%s was not setup", KC_MAP_MODULE_NAME);
    }

    if (table != NULL) {
        uint32_t i;
        /* free all entries in table */
        for (i = 0; i < table_size; ++i) {
            struct kc_map_entry *kcme, *tkcme;
            STAILQ_FOREACH_SAFE(kcme, &(table[i]), next, tkcme) {
                kc_map_entry_return(&kcme);
            }
        }
    }

    kc_map_entry_pool_destroy();
    kc_map_init = false;
}

static inline struct kcme_slh *
_get_bucket(const struct bstring *key)
{
    return &(table[hash(key->data, key->len, 0) % table_size]);
}

uint32_t
kc_map_incr(const struct bstring *key)
{
    struct kc_map_entry *kcme;
    struct kcme_slh *bucket;

    ASSERT(key->len <= MAX_KEY_LEN);

    bucket = _get_bucket(key);

    /* iterate through bucket looking for item */
    for (kcme = STAILQ_FIRST(bucket); kcme != NULL; kcme = STAILQ_NEXT(kcme, next)) {
        if ((key->len == kcme->klen) && cc_memcmp(key->data, kcme->key, key->len) == 0) {
            /* found item */
            return ++kcme->count;
        }
    }

    /* not found, insert entry */
    kcme = kc_map_entry_borrow();
    cc_memcpy(kcme->key, key->data, key->len);
    kcme->klen = key->len;
    kcme->count = 1;
    STAILQ_INSERT_HEAD(bucket, kcme, next);

    return 1;
}

void
kc_map_decr(const struct bstring *key)
{
    struct kc_map_entry *kcme, *prev;
    struct kcme_slh *bucket;

    ASSERT(key->len <= MAX_KEY_LEN);

    bucket = _get_bucket(key);

    /* iterate through bucket looking for item */
    for (prev = NULL, kcme = STAILQ_FIRST(bucket); kcme != NULL;
         prev = kcme, kcme = STAILQ_NEXT(kcme, next)) {
        if ((key->len == kcme->klen) && cc_memcmp(key->data, kcme->key, key->len) == 0) {
            /* found item */
            if (--(kcme->count) == 0) {
                /* remove entry */
                if (prev == NULL) {
                    STAILQ_REMOVE_HEAD(bucket, next);
                } else {
                    STAILQ_REMOVE_AFTER(bucket, prev, next);
                }
            }

            return;
        }
    }

    /* item not found, should never happen for hotkey detection scheme */
    ASSERT(false);
}
