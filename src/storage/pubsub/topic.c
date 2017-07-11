#include "hashtable.h"
#include "topic.h"

#include <cc_mm.h>
#include <cc_queue.h>
#include <hash/cc_lookup3.h>

#include <sysexits.h>

SLIST_HEAD(topic_slh, topic);

struct topic_ht {
    struct topic_slh *table;
    uint32_t ntopic;
    uint32_t hash_power;
};

static struct topic_ht hashtable;
static struct topic_ht *ht = &hashtable;


static struct topic_slh *
_get_bucket(const struct bstring *name)
{
    /* use the _address_ of the channel to hash */
    uint32_t hval = hash_lookup3(name->data, name->len, 0);
    return &(ht->table[hval & HASHMASK(ht->hash_power)]);
}

struct topic *
topic_get(const struct bstring *name)
{
    struct topic_slh *bucket;
    struct topic *t;

    bucket = _get_bucket(name);
    for (t = SLIST_FIRST(bucket); t != NULL; t = SLIST_NEXT(t, t_sle)) {
        log_verb("current topic name (len %d) is: %.*s", t->name.len, t->name.len, t->name.data);
        if (bstring_compare(&t->name, name) == 0) {
            return t;
        }
    }

    return NULL;
}

void
topic_put(const struct topic *t)
{
    struct topic_slh *bucket;

    ASSERT(topic_get(&t->name) == NULL);

    bucket = _get_bucket(&t->name);
    SLIST_INSERT_HEAD(bucket, (struct topic *)t, t_sle);

    ht->ntopic++;
}

void
topic_delete(const struct bstring *name)
{
    struct topic_slh *bucket;
    struct topic *t, *prev;

    bucket = _get_bucket(name);
    for (prev = NULL, t = SLIST_FIRST(bucket); t != NULL;
        prev = t, t = SLIST_NEXT(t, t_sle)) {
        if (t == NULL) {
            log_debug("topic not found for %.*s", name->len, name->data);
            return;
        }
        if (bstring_compare(&t->name, name) == 0) {
            break;
        }
    }

    if (prev == NULL) {
        SLIST_REMOVE_HEAD(bucket, t_sle);
    } else {
        SLIST_REMOVE_AFTER(prev, t_sle);
    }

    --(ht->ntopic);
}


struct topic *
topic_create(const struct bstring *name)
{
    struct topic *t;

    t = cc_alloc(sizeof(struct topic));
    if (t == NULL) {
        return NULL;
    }

    t->idx = cc_alloc(sizeof(struct index_tqh));
    if (t->idx == NULL) {
        cc_free(t);
        return NULL;
    }

    topic_reset(t);
    t->name.len = name->len;
    t->name.data = cc_alloc(t->name.len);
    if (t->name.data == NULL) {
        cc_free(t->idx);
        cc_free(t);
        return NULL;
    }
    cc_memcpy(t->name.data, name->data, name->len);

    return t;
}

void
topic_destroy(struct topic **t)
{
    ASSERT(t != NULL && *t != NULL);

    struct index_node *curr, *next;
    struct index_tqh *idx = (*t)->idx;

    /* delete all elements of the index */
    TAILQ_FOREACH_SAFE(curr, idx, i_tqe, next) {
        TAILQ_REMOVE(idx, curr, i_tqe);
        cc_free(curr);
    }
    cc_free(idx);
    cc_free((*t)->name.data);

    cc_free(*t);
    *t = NULL;
}

void
topic_reset(struct topic *t)
{
    t->name = null_bstring;
    t->nsub = 0;
    TAILQ_INIT(t->idx);
}

bool
topic_add_listener(struct topic *t, const struct listener *l)
{
    struct index_node *node;

    ASSERT(t != NULL && l != NULL);

    /* do nothing if already subscribed */
    TAILQ_FOREACH(node, t->idx, i_tqe) {
        if (node->obj == l) {
            return false;
        }
    }

    node = cc_alloc(sizeof(struct index_node));
    if (node == NULL) {
        return false;
    }
    node->obj = (struct listener *)l;

    TAILQ_INSERT_TAIL(t->idx, node, i_tqe);
    t->nsub++;

    return true;
}

void
topic_del_listener(struct topic *t, const struct listener *l)
{
    struct index_node *node;

    /* do nothing if not found */
    TAILQ_FOREACH(node, t->idx, i_tqe) {
        if (node->obj == l) {
            break;
        }
    }
    if (node == NULL) {
        return;
    }

    TAILQ_REMOVE(t->idx, node, i_tqe);
    t->nsub--;
    cc_free(node);
}

void
topic_setup(uint32_t hash_power)
{
    uint32_t i, nentry;
    struct topic_slh *table;

    ASSERT(hash_power > 0);

    ht->hash_power = hash_power;
    ht->ntopic = 0;
    nentry = HASHSIZE(ht->hash_power);

    table = cc_alloc(sizeof(*table) * nentry);
    if (table == NULL) {
        log_crit("topic setup failed: OOM");
        exit(EX_CONFIG);
    }
    ht->table = table;

    for (i = 0; i < nentry; ++i) {
        SLIST_INIT(&table[i]);
    }
}

void topic_teardown(void)
{
    if (ht->table != NULL) {
        cc_free(ht->table);
    }
}
