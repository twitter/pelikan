#include <cuckoo/bb_cuckoo.h>

#include <bb_item.h>

#include <cc_define.h>
#include <cc_log.h>
#include <cc_lookup3.h>
#include <cc_mm.h>

#include <stdlib.h>

/* TODO(yao): make the MAX_DISPLACE configurable */
#define D            4
#define MAX_DISPLACE 2

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
bool cuckoo_initialized;

#define OFFSET2ITEM(o) ((struct item *)((ds) + (o) * chunk_size))
#define RANDOM(k) (random() % k)


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

static void
cuckoo_displace(uint32_t displaced)
{
    long int i, j, k, step;
    struct bstring key;
    struct item *it;
    uint32_t offset[D];
    uint32_t path[MAX_DISPLACE + 1];
    bool ended = false;
    bool noevict = false;

    //stats_thread_incr(item_displace);

    step = 0;
    path[0] = displaced;
    while (!ended && step < MAX_DISPLACE) {
        step++;
        it = OFFSET2ITEM(displaced);
        key.len = it->klen;
        key.data = ITEM_KEY_POS(it);
        cuckoo_hash(offset, &key);

        /* first try to find an empty item */
        for (i = 0; i < D; ++i) {
            it = OFFSET2ITEM(offset[i]);
            if (!item_valid(it)) {
                log_verb("item at %p is unoccupied");

                ended = true;
                noevict = true;
                path[step] = offset[i];
                break;
            }
        }

        /* no empty item, proceed to displacement */
        if (D == i) {
            /* need to find another item that's at a different location. */
            for (i = 0, j = RANDOM(D); i < D; ++i, j = (j + 1) % D) {
                for (k = 0; k < step; k++) { /* there can be no circle */
                    if (path[k] == offset[j]) {
                        continue;
                    }
                }
                break; /* otherwise we have a candidate */
            }

            if (D == i) {
                /* all offsets are the same. no candidate for eviction. */
                log_verb("running out of displacement candidates");

                ended = true;
                --step; /* discard last step */
            }
            displaced = offset[j]; /* next displaced item */
            path[step] = displaced;
        }
    }

    if (!noevict) {
        log_verb("one item evicted during replacement");

        //stats_thread_decr(item_curr);
        //stats_thread_decr_by(data_curr, item_datalen(OFFSET2ITEM(path[step])));
        //stats_thread_incr(item_evict);
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
cuckoo_setup(size_t size, uint32_t item)
{
    chunk_size = size;
    max_item = item;
    ds = cc_zalloc(max_item * chunk_size);
    if (ds == NULL) {
        log_crit("cuckoo data store allocation failed");

        return CC_ERROR;
    }

    cuckoo_initialized = true;

    return CC_OK;
}

void
cuckoo_teardown(void)
{
    cc_free(ds);
}

struct item *
cuckoo_lookup(struct bstring *key)
{
    uint32_t offset[D];
    int i;
    struct item *it;

    ASSERT(cuckoo_initialized == true);

    cuckoo_hash(offset, key);

    for (i = 0; i < D; ++i) {
        it = OFFSET2ITEM(offset[i]);
        log_verb("item location: %p", it);
        if (cuckoo_hit(it, key)) {
            log_debug("item found: %p", it);
            return it;
        }
    }

    return NULL;
}

void
cuckoo_insert(struct bstring *key, struct val *val, rel_time_t expire)
{
    struct item *it;
    uint32_t offset[D];
    uint32_t displaced;
    int i;

    cuckoo_hash(offset, key);

    for (i = 0; i < D; ++i) {
      it = OFFSET2ITEM(offset[i]);
      if (!item_valid(it)) {
        break;
      }
    }
    log_verb("inserting into location: %p", it);

    if (D == i) {
        displaced = offset[RANDOM(D)];
        cuckoo_displace(displaced);
    }

    item_set(it, key, val, expire);
}


