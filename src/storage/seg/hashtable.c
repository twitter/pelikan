
#define XXH_INLINE_ALL

#include "hashtable.h"
#include "hash/xxhash.h"
#include "item.h"
#include "seg.h"

#include <cc_mm.h>
#include <hash/cc_murmur3.h>

#include <sys/mman.h>
#include <sysexits.h>
#include <x86intrin.h>


extern seg_metrics_st *seg_metrics;
extern bool use_cas;

static uint32_t murmur3_iv = 0x3ac5d673;
static struct hash_table lock_table;
// static struct hash_table cas_table;

// add -mlzcnt to gcc flag list
//#ifdef __LZCNT__
// uint32_t h = 63 - __lzcnt64(v);
//#else
// uint32_t h = 63 - __builtin_clzll(v);
//#endif


#define get_bucket(hv, ht) (&((ht)->table[(hv)&HASHMASK((ht)->hash_power)]))
#define get_cas(hv, ht) ((ht)->cas_table[(hv)&HASHMASK((ht)->cas_table_hp)])
#define set_cas(hv, ht)                                                        \
    __atomic_add_fetch(&((ht)->cas_table[(hv)&HASHMASK((ht)->cas_table_hp)]),  \
            1, __ATOMIC_RELAXED)

/*
 * Allocate table given size
 */
static struct item_slh *
_hashtable_alloc(uint64_t size)
{
    struct item_slh *table;
    uint32_t i;

    table = cc_alloc(sizeof(*table) * size);
#ifdef MADV_HUGEPAGE
    /* USE_HUGEPAGE */
    madvise(table, sizeof(*table) * size, MADV_HUGEPAGE);
#endif

    if (table != NULL) {
        for (i = 0; i < size; ++i) {
            SLIST_INIT(&table[i]);
        }
    }

    return table;
}

struct hash_table *
hashtable_create(uint32_t hash_power)
{
    struct hash_table *ht;
    uint64_t size;
    uint32_t n_entry;

    ASSERT(hash_power > 0);

    /* alloc struct */
    ht = cc_alloc(sizeof(struct hash_table));

    if (ht == NULL) {
        return NULL;
    }

    /* init members */
    ht->table = NULL;
    ht->hash_power = hash_power;
    size = HASHSIZE(ht->hash_power);

    /* alloc table */
    ht->table = _hashtable_alloc(size);
    if (ht->table == NULL) {
        cc_free(ht);
        return NULL;
    }

    /* create cas table */
    ht->cas_table_hp = CAS_TABLE_HASHPOWER;
    n_entry = (uint32_t)HASHSIZE(ht->cas_table_hp);
    ht->cas_table = cc_zalloc(sizeof(uint32_t) * n_entry);
    if (ht->cas_table == NULL) {
        cc_free(ht);
        return NULL;
    }

    /* create lock table */
    ht->lock_table_hp = LOCKTABLE_HASHPOWER;
    n_entry = (uint32_t)HASHSIZE(ht->lock_table_hp);
    ht->lock_table = cc_zalloc(sizeof(uint32_t) * n_entry);
    if (ht->lock_table == NULL) {
        cc_free(ht);
        return NULL;
    }

    log_info("create hash table of size %zu", size);
    return ht;
}

void
hashtable_destroy(struct hash_table **ht_p)
{
    struct hash_table *ht = *ht_p;
    if (ht != NULL && ht->table != NULL) {
        cc_free(ht->table);
        cc_free(ht->cas_table);
    }

    cc_free(*ht_p);

    *ht_p = NULL;
}

static inline uint64_t
_get_hv_murmur3(const char *key, size_t klen)
{
    uint32_t hv;

    hash_murmur3_32(key, klen, murmur3_iv, &hv);

    return (uint64_t)hv;
}

static inline uint64_t
_get_hv_xxhash(const char *key, size_t klen)
{
    return XXH3_64bits(key, klen);
    //    uint64_t hv = XXH3_64bits_dispatch(key, klen);
}


void
hashtable_put(struct item *it, struct hash_table *ht)
{
    struct item_slh *bucket;

    ASSERT(hashtable_get(item_key(it), it->klen, ht, NULL) == NULL);

    uint64_t hv = get_hv(item_key(it), it->klen);

    bucket = get_bucket(hv, ht);

    if (use_cas){
        /* update cas_table */
        set_cas(hv, ht);
    }


    SLIST_INSERT_HEAD(bucket, it, hash_next);
//    SLIST_NEXT((elm), field) = SLIST_FIRST((head));
//    SLIST_FIRST((head)) = (elm);


    INCR(seg_metrics, hash_insert);

}

bool
hashtable_delete(const char *key, uint32_t klen, struct hash_table *ht,
        bool try_del, struct item **it_p)
{
    struct item_slh *bucket;
    struct item *it, *prev;

    uint64_t hv = get_hv(key, klen);

    bucket = get_bucket(hv, ht);

    for (prev = NULL, it = SLIST_FIRST(bucket); it != NULL;
            prev = it, it = SLIST_NEXT(it, hash_next)) {
        INCR(seg_metrics, hash_traverse);

        /* iterate through bucket to find item to be removed */
        if ((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            break;
        }
    }

    if (it != NULL) {
        if (prev == NULL) {
            SLIST_REMOVE_HEAD(bucket, hash_next);
        } else {
            SLIST_REMOVE_AFTER(prev, hash_next);
        }

        if (it_p != NULL) {
            *it_p = it;
        }

        INCR(seg_metrics, hash_remove);

        return true;
    } else {
        ASSERT(try_del);

        return false;
    }
}

/*
 * delete the hashtable entry only if item is the up-to-date/valid item
 */
bool
hashtable_delete_it(struct item *oit, struct hash_table *ht)
{
    struct item_slh *bucket;
    struct item *it, *prev;

    uint64_t hv = get_hv(item_key(oit), item_nkey(oit));

    bucket = get_bucket(hv, ht);

    for (prev = NULL, it = SLIST_FIRST(bucket); it != NULL;
            prev = it, it = SLIST_NEXT(it, hash_next)) {
        INCR(seg_metrics, hash_traverse);

        /* iterate through bucket to find item to be removed */
        if (it == oit) {
            /* found item */
            if (prev == NULL) {
                SLIST_REMOVE_HEAD(bucket, hash_next);
            } else {
                SLIST_REMOVE_AFTER(prev, hash_next);
            }

            INCR(seg_metrics, hash_remove);

            return true;
        }
    }
    return false;
}

struct item *
hashtable_get(
        const char *key, uint32_t klen, struct hash_table *ht, uint64_t *cas)
{
    struct item_slh *bucket;
    struct item *it;

    ASSERT(key != NULL);
    ASSERT(klen != 0);

    INCR(seg_metrics, hash_lookup);

    uint64_t hv = get_hv(key, klen);

    bucket = get_bucket(hv, ht);

    if (cas) {
        *cas = get_cas(hv, ht);
    }

    /* iterate through bucket looking for item */
    for (it = SLIST_FIRST(bucket); it != NULL; it = SLIST_NEXT(it, hash_next)) {
        INCR(seg_metrics, hash_traverse);

        if ((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            return it;
        }
    }

    return NULL;
}

/*
 * Expand the hashtable to the next power of 2.
 * This is an expensive operation and should _not_ be used in production or
 * during latency-related tests. It is included mostly for simulation around
 * the storage component.
 */
struct hash_table *
hashtable_double(struct hash_table *ht)
{
    struct hash_table *new_ht;
    uint32_t new_hash_power;
    uint64_t new_size;
    uint64_t hv;

    new_hash_power = ht->hash_power + 1;
    new_size = HASHSIZE(new_hash_power);

    new_ht = hashtable_create(new_size);
    if (new_ht == NULL) {
        return ht;
    }

    /* copy to new hash table */
    for (uint32_t i = 0; i < HASHSIZE(ht->hash_power); ++i) {
        struct item *it, *next;
        struct item_slh *bucket, *new_bucket;

        bucket = &ht->table[i];
        SLIST_FOREACH_SAFE(it, bucket, hash_next, next)
        {
            hv = get_hv(item_key(it), item_nkey(it));
            new_bucket = get_bucket(hv, new_ht);
            SLIST_REMOVE(bucket, it, item, hash_next);
            SLIST_INSERT_HEAD(new_bucket, it, hash_next);
        }
    }

    hashtable_destroy(&ht);

    return new_ht;
}
