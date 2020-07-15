
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
extern struct hash_table *hash_table;

static uint32_t murmur3_iv = 0x3ac5d673;


#define get_hv(key, klen) _get_hv_xxhash(key, klen)
//#define get_hv(key, klen) _get_hv_murmur3(key, klen)

#define get_bucket(hv, ht) (&((ht)->table[(hv)&HASHMASK((ht)->hash_power)]))
#define get_cas(hv, ht) ((ht)->cas_table[(hv)&HASHMASK((ht)->cas_table_hp)])
#define set_cas(hv, ht)                                                        \
    __atomic_add_fetch(&((ht)->cas_table[(hv)&HASHMASK((ht)->cas_table_hp)]),  \
            1, __ATOMIC_RELAXED)

#define mtx_lock(hv, ht)                                                       \
    do {                                                                       \
        int status = pthread_mutex_lock(                                       \
                &(ht)->mtx_table[(hv)&HASHMASK((ht)->lock_table_hp)]);         \
        ASSERT(status == 0);                                                   \
    } while (0)

#define mtx_unlock(hv, ht)                                                     \
    do {                                                                       \
        int status = pthread_mutex_unlock(                                     \
                &(ht)->mtx_table[(hv)&HASHMASK((ht)->lock_table_hp)]);         \
        ASSERT(status == 0);                                                   \
    } while (0)

/*
#undef mtx_lock
#undef mtx_unlock

#define mtx_lock(hv, ht)                                                       \
    do {                                                                       \
        pthread_mutex_lock(                                                    \
                &(ht)->mtx_table[(hv)&HASHMASK((ht)->lock_table_hp)]);         \
        printf("lock %s:%d\n", __FUNCTION__, __LINE__);                        \
    } while (0)
#define mtx_unlock(hv, ht)                                                     \
    do {                                                                       \
        pthread_mutex_unlock(                                                  \
                &(ht)->mtx_table[(hv)&HASHMASK((ht)->lock_table_hp)]);         \
        printf("unlock %s:%d\n", __FUNCTION__, __LINE__);                      \
    } while (0)
*/


//#define mtx_lock(hv, ht)
//#define mtx_unlock(hv, ht)

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
        cc_free(ht->table);
        cc_free(ht);
        return NULL;
    }

    /* create lock table */
    ht->lock_table_hp = LOCKTABLE_HASHPOWER;
    n_entry = (uint32_t)HASHSIZE(ht->lock_table_hp);
    ht->lock_table = cc_zalloc(sizeof(uint32_t) * n_entry);
    if (ht->lock_table == NULL) {
        cc_free(ht->table);
        cc_free(ht->cas_table);
        cc_free(ht);
        return NULL;
    }

    /* create mtx table */
    ht->lock_table_hp = LOCKTABLE_HASHPOWER;
    n_entry = (uint32_t)HASHSIZE(ht->lock_table_hp);
    ht->mtx_table = cc_alloc(sizeof(pthread_mutex_t) * n_entry);
    if (ht->mtx_table == NULL) {
        cc_free(ht->table);
        cc_free(ht->cas_table);
        cc_free(ht->lock_table);
        cc_free(ht);
        return NULL;
    }

    for (uint32_t i = 0; i < n_entry; i++) {
        pthread_mutex_init(&ht->mtx_table[i], NULL);
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

    uint64_t hv = get_hv(item_key(it), it->klen);

    bucket = get_bucket(hv, ht);

    if (use_cas) {
        /* update cas_table */
        set_cas(hv, ht);
    }

    mtx_lock(hv, ht);

    SLIST_INSERT_HEAD(bucket, it, hash_next);
    mtx_unlock(hv, ht);


    INCR(seg_metrics, hash_insert);
}


bool
hashtable_del_and_put(struct item *it, struct hash_table *ht)
{
    struct item_slh *bucket;
    struct item *curr, *prev;
    bool found_old = false;

    uint64_t hv = get_hv(item_key(it), it->klen);

    bucket = get_bucket(hv, ht);

    if (use_cas) {
        /* update cas_table */
        set_cas(hv, ht);
    }

    uint32_t klen = item_nkey(it);
    char *key = item_key(it);

    mtx_lock(hv, ht);

    /* now delete */
    for (prev = NULL, curr = SLIST_FIRST(bucket); curr != NULL;
            prev = curr, curr = SLIST_NEXT(curr, hash_next)) {
        INCR(seg_metrics, hash_traverse);

        /* iterate through bucket to find item to be removed */
        if ((klen == curr->klen) && cc_memcmp(key, item_key(curr), klen) == 0) {
            /* found item */
            if (prev == NULL) {
                SLIST_REMOVE_HEAD(bucket, hash_next);
            } else {
                SLIST_REMOVE_AFTER(prev, hash_next);
            }
            found_old = true;

            item_free(curr);

            INCR(seg_metrics, hash_remove);

            break;
        }
    }

    /* now insert */
    SLIST_INSERT_HEAD(bucket, it, hash_next);

    mtx_unlock(hv, ht);


    INCR(seg_metrics, hash_insert);

    return found_old;
}
bool
hashtable_delete(
        const char *key, uint32_t klen, struct hash_table *ht, bool try_del)
{
    struct item_slh *bucket;
    struct item *it, *prev;
    bool deleted = false;

    uint64_t hv = get_hv(key, klen);

    bucket = get_bucket(hv, ht);

    mtx_lock(hv, ht);

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
        deleted = true;

        /* this has to be done wihtin critical section,
         * we need to either move the lock to item.c
         * or do item_free here, I prefer the later because this makes
         * the critical section shorter */

        item_free(it);

        INCR(seg_metrics, hash_remove);

    } else {
        ASSERT(try_del);
    }

    mtx_unlock(hv, ht);

    return deleted;
}

/*
 * delete the hashtable entry only if item is the up-to-date/valid item
 */
bool
hashtable_delete_it(struct item *oit, struct hash_table *ht)
{
    struct item_slh *bucket;
    struct item *it, *prev;
    bool deleted = false;

    uint64_t hv = get_hv(item_key(oit), item_nkey(oit));

    bucket = get_bucket(hv, ht);

    mtx_lock(hv, ht);

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

            deleted = true;

            item_free(oit);

            INCR(seg_metrics, hash_remove);

            break;
        }
    }

    mtx_unlock(hv, ht);

    return deleted;
}

struct item *
hashtable_get(
        const char *key, uint32_t klen, struct hash_table *ht, uint64_t *cas)
{
    struct item_slh *bucket;
    struct item *it = NULL;

    ASSERT(key != NULL);
    ASSERT(klen != 0);

    INCR(seg_metrics, hash_lookup);

    uint64_t hv = get_hv(key, klen);

    bucket = get_bucket(hv, ht);

    if (cas) {
        *cas = get_cas(hv, ht);
    }

    mtx_lock(hv, ht);

    /* iterate through bucket looking for item */
    for (it = SLIST_FIRST(bucket); it != NULL; it = SLIST_NEXT(it, hash_next)) {
        INCR(seg_metrics, hash_traverse);

        if ((klen == it->klen) && cc_memcmp(key, item_key(it), klen) == 0) {
            /* found item */
            break;
        }
    }

    mtx_unlock(hv, ht);

    return it;
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

void
hashtable_print_chain_depth_hist(void)
{
#define MAX_DEPTH 2000
    struct hash_table *ht = hash_table;
    uint64_t n_item = HASHSIZE(ht->hash_power);
    uint64_t n_active_item = 0, n_active_bucket = 0;
    uint64_t i;
    uint32_t depth, max_depth = 0;
    struct item *it;
    uint32_t hist[MAX_DEPTH];


    for (i = 0; i < n_item; i++) {
        depth = 0;
        for (it = SLIST_FIRST(&ht->table[i]); it != NULL;
                it = SLIST_NEXT(it, hash_next)) {
            depth += 1;
            n_active_item += 1;
        }
        if (depth > max_depth)
            max_depth = depth;
        if (depth > MAX_DEPTH)
            depth = MAX_DEPTH - 1;
        hist[depth] += 1;
        n_active_bucket += 1;
    }

    double load = (double)n_active_item / n_item;
    printf("hashtable hash chain depth hist, load %.2lf - depth: count\n",
            load);
    uint32_t print_cnt = 0;
    for (i = 0; i < max_depth; i++) {
        if (hist[i] != 0) {
            print_cnt += 1;
            printf("%" PRIu64 ": %" PRIu32 "(%.4lf), ", i, hist[i],
                    (double)hist[i] / n_active_bucket);
            if (print_cnt % 20 == 0)
                printf("\n");
        }
    }

    printf("\n");
}


/*
 * given a hash value array of items in the same bucket,
 * check whether there are collisions if we use the first n_bit as tag
 *
 */
static inline bool
has_duplicate(uint64_t hv, int n_bit)
{
}

void
hashtable_print_tag_collision_hist(void)
{
    struct hash_table *ht = hash_table;
    uint64_t n_item = HASHSIZE(ht->hash_power);
    uint64_t n_active_item = 0, n_active_bucket = 0;
    uint64_t i;
    int tag_size; /* the size of tag in bits */
    uint32_t depth, max_depth = 0;
    struct item *it;
    bool bucket_has_tag_collision;
    uint32_t hv[MAX_DEPTH];
    uint64_t tag_collision_cnt[32];

    for (i = 0; i < 32; i++) {
        tag_collision_cnt[i] = 0;
    }

    for (i = 0; i < n_item; i++) {
        depth = 0;
        for (it = SLIST_FIRST(&ht->table[i]); it != NULL;
                it = SLIST_NEXT(it, hash_next)) {
            hv[depth] = _get_hv_xxhash(item_key(it), it->klen);
            depth += 1;
        }

        for (tag_size = 1; tag_size < 32; tag_size++) {
            bucket_has_tag_collision = has_duplicate(hv, tag_size);
            if (bucket_has_tag_collision) {
                tag_collision_cnt[tag_size] += 1;
            }
        }
        n_active_bucket += 1;
    }

    double load = (double)n_active_item / n_item;
    printf("hashtable tag collision hist, load %.2lf, "
           "tag size (in bits): fraction of buckets have tag collisions\n",
            load);

    for (i = 1; i < 32; i++) {
        printf("%" PRIu64 ": %.4lf, ", i,
                (double)tag_collision_cnt[i] / n_active_bucket);
    }
}
