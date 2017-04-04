#include "hashtable.h"
#include "topic.h"

#include <cc_mm.h>
#include <cc_queue.h>
#include <hash/cc_lookup3.h>

static struct topic_slh *
_ht_alloc(uint32_t nentry)
{
    struct topic_slh *table;
    uint32_t i;

    table = cc_alloc(sizeof(*table) * nentry);

    if (table != NULL) {
        for (i = 0; i < nentry; ++i) {
            SLIST_INIT(&table[i]);
        }
    }

    return table;
}

struct topic_ht *
topic_ht_create(uint32_t hash_power)
{
    struct topic_ht *ht;
    uint32_t nentry;

    ASSERT(hash_power > 0);

    ht = cc_alloc(sizeof(struct topic_ht));
    if (ht == NULL) {
        return NULL;
    }

    ht->hash_power = hash_power;
    ht->ntopic = 0;
    nentry = HASHSIZE(ht->hash_power);
    ht->table = _ht_alloc(nentry);
    if (ht->table == NULL) {
        cc_free(ht);
        return NULL;
    }

    return ht;
}

void
topic_ht_destroy(struct topic_ht **ht)
{
    ASSERT(ht != NULL);

    if (*ht != NULL && (*ht)->table != NULL) {
        cc_free((*ht)->table);
    }

    cc_free(*ht);
    *ht = NULL;
}


static struct topic_slh *
_get_bucket(const struct bstring *name, struct topic_ht *ht)
{
    /* use the _address_ of the channel to hash */
    uint32_t hval = hash_lookup3(name->data, name->len, 0);
    return &(ht->table[hval & HASHMASK(ht->hash_power)]);
}

struct topic *
topic_ht_get(const struct bstring *name, struct topic_ht *ht)
{
    struct topic_slh *bucket;
    struct topic *t;

    bucket = _get_bucket(name, ht);
    for (t = SLIST_FIRST(bucket); t != NULL; t = SLIST_NEXT(t, t_sle)) {
        if (bstring_compare(&t->name, name) == 0) {
            return t;
        }
    }

    return NULL;
}

void
topic_ht_put(const struct topic *t, struct topic_ht *ht)
{
    struct topic_slh *bucket;

    ASSERT(topic_ht_get(&t->name, ht) == NULL);

    bucket = _get_bucket(&t->name, ht);
    SLIST_INSERT_HEAD(bucket, (struct topic *)t, t_sle);

    ht->ntopic++;
}

void
topic_ht_delete(const struct bstring *name, struct topic_ht *ht)
{
    struct topic_slh *bucket;
    struct topic *t, *prev;

    bucket = _get_bucket(name, ht);
    for (prev = NULL, t = SLIST_FIRST(bucket); t != NULL;
        prev = t, t = SLIST_NEXT(t, t_sle)) {
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
    t->name = *name;

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
