#include <storage/cuckoo/bb_cuckoo.h>

#include <bb_stats.h>

#include <cc_define.h>
#include <cc_log.h>
#include <cc_lookup3.h>
#include <cc_mm.h>

#include <stdlib.h>

#define D            4

uint32_t cuckoo_policy = CUCKOO_POLICY;

static uint32_t iv[D] = {
    /* these numbers can be picked arbitrarily as long as they are different */
    0x3ac5d673,
    0x6d7839d0,
    0x2b581cf5,
    0x4dd2be0a
};

static void* ds; /* data store is also the hash table */
static size_t chunk_size;
static uint32_t max_item;
static bool cuckoo_init; /* need to make sure memory has been pre-allocate */

#define OFFSET2ITEM(o) ((struct item *)((ds) + (o) * chunk_size))
#define RANDOM(k) (random() % k)

#define ITEM_METRICS_INCR(it)   do {                        \
    INCR(item_curr);                                        \
    INCR_N(item_key_curr, item_klen(it));                   \
    INCR_N(item_val_curr, item_vlen(it));                   \
    INCR_N(item_data_curr, item_datalen(it));               \
} while(0)

#define ITEM_METRICS_DECR(it)   do {                        \
    DECR(item_curr);                                        \
    DECR_N(item_key_curr, item_klen(it));                   \
    DECR_N(item_val_curr, item_vlen(it));                   \
    DECR_N(item_data_curr, item_datalen(it));               \
} while(0)

static inline uint32_t vlen(struct val *val)
{
    if (val->type == VAL_TYPE_INT) {
        return sizeof(uint64_t);
    } else if (val->type == VAL_TYPE_STR) {
        return val->vstr.len;
    } else {
        NOT_REACHED();
        return UINT32_MAX;
    }
}

static bool
cuckoo_hit(struct item *it, struct bstring *key)
{
    log_verb("valid? %d; match? %d", item_valid(it), item_matched(it, key));

    return item_valid(it) && item_matched(it, key);
}

static void
cuckoo_hash(uint32_t offset[], struct bstring *key)
{
    int i;

    for (i = 0; i < D; ++i) {
        offset[i] = hashlittle(key->data, key->len, iv[i]) % max_item;
    }

    return;
}

static inline uint32_t
_select_candidate(const uint32_t offset[])
{
    uint32_t selected = offset[0];

    if (cuckoo_policy == CUCKOO_POLICY_RANDOM) {
        selected = offset[RANDOM(D)];
    } else if (cuckoo_policy == CUCKOO_POLICY_EXPIRE) {
        rel_time_t expire, min = UINT32_MAX; /* legal ts should < UINT32_MAX */
        uint32_t i;

        for (i = 0; i < D; ++i) {
            expire = item_expire(OFFSET2ITEM(offset[i]));
            if (expire < min) {
                min = expire;
                selected = offset[i];
            }
        }
    } else {
        NOT_REACHED();
    }

    log_verb("selected offset: %"PRIu32, selected);

    return selected;
}

/* sorts candidate offsets based on policy */
static void
_sort_candidate(uint32_t ordered[], const uint32_t offset[])
{
    uint32_t i;
    /* offset always holds D elements */

    if (cuckoo_policy == CUCKOO_POLICY_RANDOM) {
        /* only pick the first item randomly, and "sort" the rest linearly */
        uint32_t j = RANDOM(D);

        for (i = 0; i < D; ++i, j = (j + 1) % D) {
            ordered[i] = offset[j];
        }
    } else if (cuckoo_policy == CUCKOO_POLICY_EXPIRE) {
        rel_time_t expire[D];

        for (i = 0; i < D; ++i) {
            uint32_t j = i;
            rel_time_t te;
            uint32_t to;

            /* basically an insert sort */
            expire[i] = item_expire(OFFSET2ITEM(offset[i]));
            ordered[i] = offset[i];
            while (j > 0 && expire[j] < expire[j - 1]) {
                /* swap */
                te = expire[j - 1];
                expire[j - 1] = expire[j];
                expire[j] = te;
                to = ordered[j - 1];
                ordered[j - 1] = ordered[j];
                ordered[j] = to;
                j--;
            }
        }
    } else {
        NOT_REACHED();
    }
}

static void
cuckoo_displace(uint32_t displaced)
{
    long int i, j, k, step;
    struct bstring key;
    struct item *it;
    /* both offset and ordered may have duplicates, treat with care */
    uint32_t offset[D];
    uint32_t ordered[D];
    uint32_t path[CUCKOO_DISPLACE + 1];
    bool ended = false;
    bool evict = true;

    step = 0;
    path[0] = displaced;
    while (!ended && step < CUCKOO_DISPLACE) {
        step++;
        it = OFFSET2ITEM(displaced);
        key.len = it->klen;
        key.data = ITEM_KEY_POS(it);
        cuckoo_hash(offset, &key);

        /* first try to find an empty item */
        for (i = 0; i < D; ++i) {
            it = OFFSET2ITEM(offset[i]);
            if (item_valid(it)) {
                continue;
            }
            log_verb("item at %p is unoccupied", it);

            ended = true;
            evict = false;
            path[step] = offset[i];
            INCR(item_displace);

            if (item_expired(it)) {
                INCR(item_expire);
                ITEM_METRICS_DECR(it);
            }

            break;
        }

        /* no empty item, proceed to displacement */
        if (D == i) {
            _sort_candidate(ordered, offset);
            /* need to find another item that's at a different location. */
            for (j = 0; j < D; ++j) {
                for (k = 0; k < step; k++) { /* there can be no circle */
                    if (path[k] == ordered[j]) {
                        continue;
                    }
                }
                break; /* otherwise we have a candidate */
            }

            if (D == j) {
                /* all offsets are the same. no candidate for eviction. */
                log_debug("running out of displacement candidates");

                ended = true;
                --step; /* discard last step */
            } else {
                INCR(item_displace);
                displaced = ordered[j]; /* next displaced item */
                path[step] = displaced;
            }
        }
    }

    if (evict) {
        log_verb("one item evicted during replacement");

        it = OFFSET2ITEM(path[step]);
        INCR(item_evict);
        ITEM_METRICS_DECR(it);
    }

    /* move items along the path we have found */
    for (i = step; i > 0; --i) {
        log_vverb("move item at %p to %p", OFFSET2ITEM(path[i - 1]),
                OFFSET2ITEM(path[i]));

        cc_memcpy(OFFSET2ITEM(path[i]), OFFSET2ITEM(path[i - 1]), chunk_size);
    }

    OFFSET2ITEM(path[0])->expire = 0;
    return;
}


rstatus_t
cuckoo_setup(size_t size, uint32_t item, uint32_t policy)
{
    if (cuckoo_init) {
        log_error("cuckoo has already been setup, aborting");

        return CC_ERROR;
    }

    chunk_size = size;
    max_item = item;
    ds = cc_zalloc(max_item * chunk_size);
    if (ds == NULL) {
        log_crit("cuckoo data store allocation failed");

        return CC_ERROR;
    }
    policy = policy;
    cuckoo_init = true;

    return CC_OK;
}

void
cuckoo_teardown(void)
{
    if (cuckoo_init) {
        cc_free(ds);
        cuckoo_init = false;
    } else {
        log_warn("cuckoo has never been setup");
    }
}

struct item *
cuckoo_lookup(struct bstring *key)
{
    uint32_t offset[D];
    int i;
    struct item *it;

    ASSERT(cuckoo_init == true);

    cuckoo_hash(offset, key);

    for (i = 0; i < D; ++i) {
        it = OFFSET2ITEM(offset[i]);
        if (cuckoo_hit(it, key)) {
        log_verb("found item at location: %p", it);
            return it;
        }
    }

    log_verb("item not found");

    return NULL;
}

/* insert applies to a key that doesn't exist validly in our array */
rstatus_t
cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire)
{
    struct item *it;
    uint32_t offset[D];
    uint32_t displaced;
    int i;

    if (key->len + vlen(val) + ITEM_OVERHEAD > chunk_size) {
        log_warn("key value exceed chunk size %zu: key len %"PRIu32", vlen %"
                PRIu32", item overhead %u", chunk_size, key->len, vlen(val),
                ITEM_OVERHEAD);

        return CC_ERROR;
    }

    cuckoo_hash(offset, key);

    for (i = 0; i < D; ++i) {
        it = OFFSET2ITEM(offset[i]);
        if (item_valid(it)) {
            continue;
        }
        if (item_expired(it)) {
            INCR(item_expire);
            ITEM_METRICS_DECR(it);
        }

        log_verb("inserting into location: %p", it);

        break;
    }

    if (D == i) {
        displaced = _select_candidate(offset);
        it = OFFSET2ITEM(displaced); /* we are writing into this item */
        cuckoo_displace(displaced);
    }

    item_set(it, key, val, expire);
    INCR(item_insert);
    ITEM_METRICS_INCR(it);

    return CC_OK;
}

rstatus_t
cuckoo_update(struct item *it, struct val *val, rel_time_t expire)
{
    ASSERT(it != NULL);

    if (item_klen(it) + vlen(val) + ITEM_OVERHEAD > chunk_size) {
        log_warn("key value exceed chunk size");

        return CC_ERROR;
    }

    DECR_N(item_val_curr, item_vlen(it));
    DECR_N(item_data_curr, item_vlen(it));
    item_update(it, val, expire);
    INCR_N(item_val_curr, item_vlen(it));
    INCR_N(item_data_curr, item_vlen(it));

    return CC_OK;
}

bool
cuckoo_delete(struct bstring *key)
{
    struct item *it;

    it = cuckoo_lookup(key);

    if (it != NULL) {
        INCR(item_delete);
        ITEM_METRICS_DECR(it);
        item_delete(it);
        log_verb("deleting item at location %p", it);

        return true;
    } else {
        log_verb("item not found");

        return false;
    }
}
