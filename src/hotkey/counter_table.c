#include "counter_table.h"

#include "constant.h"

#include <cc_debug.h>
#include <cc_hash.h>
#include <cc_mm.h>
#include <cc_pool.h>

#define COUNTER_TABLE_MODULE_NAME "hotkey::counter_table"

struct counter_table_entry {
    STAILQ_ENTRY(counter_table_entry) next; /* entry in hash table or pool */

    char                              key[MAX_KEY_LEN];
    uint32_t                          nkey;
    uint32_t                          count;
};

STAILQ_HEAD(cte_slh, counter_table_entry);

static struct cte_slh *table = NULL;
static uint32_t table_size = 0; /* number of buckets in table */
static bool counter_table_init = false;

FREEPOOL(cte_pool, cteq, counter_table_entry);
static struct cte_pool ctep;
static bool ctep_init = false;

static void
counter_table_entry_reset(struct counter_table_entry *cte)
{
    cte->nkey = 0;
    cte->count = 0;
}

static struct counter_table_entry *
counter_table_entry_create(void)
{
    struct counter_table_entry *cte = cc_alloc(sizeof(*cte));

    if (cte == NULL) {
        return NULL;
    }

    counter_table_entry_reset(cte);

    return cte;
}

static void
counter_table_entry_destroy(struct counter_table_entry **counter_table_entry)
{
    struct counter_table_entry *cte = *counter_table_entry;
    ASSERT(cte != NULL);

    cc_free(cte);
    *counter_table_entry = NULL;
}

static void
counter_table_entry_pool_destroy(void)
{
    struct counter_table_entry *cte, *tcte;

    if (!ctep_init) {
        log_warn("counter_table_entry pool was not created, ignore");
    }

    log_info("destroying counter_table_entry pool: free %"PRIu32, ctep.nfree);

    FREEPOOL_DESTROY(cte, tcte, &ctep, next, counter_table_entry_destroy);
    ctep_init = false;
}

static struct counter_table_entry *
counter_table_entry_borrow(void)
{
    struct counter_table_entry *cte;

    FREEPOOL_BORROW(cte, &ctep, next, counter_table_entry_create);
    if (cte == NULL) {
        log_debug("borrow counter_table_entry failed: OOM");
        return NULL;
    }
    counter_table_entry_reset(cte);

    return cte;
}

static void
counter_table_entry_return(struct counter_table_entry **counter_table_entry)
{
    struct counter_table_entry *cte = *counter_table_entry;

    if (cte == NULL) {
        return;
    }

    FREEPOOL_RETURN(cte, &ctep, next);

    *counter_table_entry = NULL;
}

static void
counter_table_entry_pool_create(uint32_t max)
{
    struct counter_table_entry *cte;

    if (ctep_init) {
        log_warn("counter_table_entry pool has already been created, re-creating");
        counter_table_entry_pool_destroy();
    }

    log_info("creating counter_table_entry pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&ctep, max);
    ctep_init = true;

    FREEPOOL_PREALLOC(cte, &ctep, max, next, counter_table_entry_create);
    if (ctep.nfree < max) {
        log_crit("cannot preallocate counter_table_entry pool, OOM. abort");
        exit(EXIT_FAILURE);
    }
}

void
counter_table_setup(uint32_t size, uint32_t poolsize)
{
    uint32_t i;

    log_info("Set up the %s module", COUNTER_TABLE_MODULE_NAME);

    if (counter_table_init) {
        log_warn("%s has already been setup, ignore", COUNTER_TABLE_MODULE_NAME);
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

    counter_table_entry_pool_create(poolsize);
}

void
counter_table_teardown(void)
{
    log_info("Tear down the %s module", COUNTER_TABLE_MODULE_NAME);

    if (!counter_table_init) {
        log_warn("%s was not setup", COUNTER_TABLE_MODULE_NAME);
    }

    if (table != NULL) {
        uint32_t i;
        /* free all entries in table */
        for (i = 0; i < table_size; ++i) {
            struct counter_table_entry *cte, *tcte;
            STAILQ_FOREACH_SAFE(cte, &(table[i]), next, tcte) {
                counter_table_entry_return(&cte);
            }
        }
    }

    counter_table_entry_pool_destroy();
    counter_table_init = false;
}

static inline struct cte_slh *
_get_bucket(const char *key, uint32_t nkey)
{
    return &(table[hash(key, nkey, 0) % table_size]);
}

uint32_t
counter_table_incr(char *key, uint32_t nkey)
{
    struct counter_table_entry *cte;
    struct cte_slh *bucket;

    ASSERT(nkey <= MAX_KEY_LEN);

    bucket = _get_bucket(key, nkey);

    /* iterate through bucket looking for item */
    for (cte = STAILQ_FIRST(bucket); cte != NULL; cte = STAILQ_NEXT(cte, next)) {
        if ((nkey == cte->nkey) && cc_memcmp(key, cte->key, nkey) == 0) {
            /* found item */
            return ++cte->count;
        }
    }

    /* not found, insert entry */
    cte = counter_table_entry_borrow();
    cc_memcpy(cte->key, key, nkey);
    cte->nkey = nkey;
    cte->count = 1;
    STAILQ_INSERT_HEAD(bucket, cte, next);

    return 1;
}

void
counter_table_decr(char *key, uint32_t nkey)
{
    struct counter_table_entry *cte, *prev;
    struct cte_slh *bucket;

    ASSERT(nkey <= MAX_KEY_LEN);

    bucket = _get_bucket(key, nkey);

    /* iterate through bucket looking for item */
    for (prev = NULL, cte = STAILQ_FIRST(bucket); cte != NULL;
         prev = cte, cte = STAILQ_NEXT(cte, next)) {
        if ((nkey == cte->nkey) && cc_memcmp(key, cte->key, nkey) == 0) {
            /* found item */
            if (--(cte->count) == 0) {
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
