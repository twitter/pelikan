#include "hashtable.h"
#include "listener.h"

#include <cc_mm.h>
#include <cc_queue.h>
#include <hash/cc_lookup3.h>

static struct listener_slh *
_ht_alloc(uint32_t nentry)
{
    struct listener_slh *table;
    uint32_t i;

    table = cc_alloc(sizeof(*table) * nentry);

    if (table != NULL) {
        for (i = 0; i < nentry; ++i) {
            SLIST_INIT(&table[i]);
        }
    }

    return table;
}

struct listener_ht *
listener_ht_create(uint32_t hash_power)
{
    struct listener_ht *ht;
    uint32_t nentry;

    ASSERT(hash_power > 0);

    ht = cc_alloc(sizeof(struct listener_ht));
    if (ht == NULL) {
        return NULL;
    }

    ht->hash_power = hash_power;
    ht->nlistener = 0;
    nentry = HASHSIZE(ht->hash_power);
    ht->table = _ht_alloc(nentry);
    if (ht->table == NULL) {
        cc_free(ht);
        return NULL;
    }

    return ht;
}

void listener_ht_destroy(struct listener_ht **ht)
{
    ASSERT(ht != NULL);

    if (*ht != NULL && (*ht)->table != NULL) {
        cc_free((*ht)->table);
    }

    cc_free(*ht);
    *ht = NULL;
}


static struct listener_slh *
_get_bucket(const channel_p ch, struct listener_ht *ht)
{
    /* use the _address_ of the channel to hash */
    uint32_t hval = hash_lookup3((char *)&ch, sizeof(channel_p), 0);
    return &(ht->table[hval & HASHMASK(ht->hash_power)]);
}

struct listener *
listener_ht_get(const channel_p ch, struct listener_ht *ht)
{
    struct listener_slh *bucket;
    struct listener *l;

    bucket = _get_bucket(ch, ht);
    for (l = SLIST_FIRST(bucket); l != NULL; l = SLIST_NEXT(l, l_sle)) {
        if (l->ch == ch) {
            return l;
        }
    }

    return NULL;
}

void
listener_ht_put(const struct listener *l, struct listener_ht *ht)
{
    struct listener_slh *bucket;

    ASSERT(listener_ht_get(l->ch, ht) == NULL);

    bucket = _get_bucket(l->ch, ht);
    SLIST_INSERT_HEAD(bucket, (struct listener *)l, l_sle);

    ht->nlistener++;
}

void
listener_ht_delete(const channel_p ch, struct listener_ht *ht)
{
    struct listener_slh *bucket;
    struct listener *l, *prev;

    bucket = _get_bucket(ch, ht);
    for (prev = NULL, l = SLIST_FIRST(bucket); l != NULL;
        prev = l, l = SLIST_NEXT(l, l_sle)) {
        if (l->ch == ch) {
            break;
        }
    }

    if (prev == NULL) {
        SLIST_REMOVE_HEAD(bucket, l_sle);
    } else {
        SLIST_REMOVE_AFTER(prev, l_sle);
    }

    --(ht->nlistener);
}


struct listener *
listener_create(channel_p ch, channel_handler_st *handler)
{
    struct listener *l;

    l = cc_alloc(sizeof(struct listener));
    if (l == NULL) {
        return NULL;
    }

    l->idx = cc_alloc(sizeof(struct index_tqh));
    if (l->idx == NULL) {
        cc_free(l);
        return NULL;
    }

    listener_reset(l);
    l->ch = ch;
    l->handler = handler;

    return l;
}

void
listener_destroy(struct listener **l)
{
    ASSERT(l != NULL && *l != NULL);

    struct index_node *curr, *next;
    struct index_tqh *idx = (*l)->idx;

    /* delete all elements of the index */
    TAILQ_FOREACH_SAFE(curr, idx, i_tqe, next) {
        TAILQ_REMOVE(idx, curr, i_tqe);
        cc_free(curr);
    }
    cc_free(idx);

    cc_free(*l);
    *l = NULL;
}

void
listener_reset(struct listener *l)
{
    l->ch = NULL;
    l->handler = NULL;
    l->ntopic = 0;
    TAILQ_INIT(l->idx);
}

bool
listener_add_topic(struct listener *l, const struct topic *t)
{
    struct index_node *node;

    ASSERT(l != NULL && t != NULL);

    /* do nothing if already subscribed */
    TAILQ_FOREACH(node, l->idx, i_tqe) {
        if (node->obj == t) {
            return false;
        }
    }

    node = cc_alloc(sizeof(struct index_node));
    if (node == NULL) {
        return false;
    }
    node->obj = (struct topic *)t;

    TAILQ_INSERT_TAIL(l->idx, node, i_tqe);
    l->ntopic++;

    return true;
}

void
listener_del_topic(struct listener *l, const struct topic *t)
{
    struct index_node *node;

    /* do nothing if not found */
    TAILQ_FOREACH(node, l->idx, i_tqe) {
        if (node->obj == t) {
            break;
        }
    }
    if (node == NULL) {
        return;
    }

    TAILQ_REMOVE(l->idx, node, i_tqe);
    l->ntopic--;
}
