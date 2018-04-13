#include "hashtable.h"
#include "listener.h"

#include <cc_mm.h>
#include <cc_queue.h>
#include <hash/cc_lookup3.h>

#include <sysexits.h>

SLIST_HEAD(listener_slh, listener);

struct listener_ht {
    struct listener_slh *table;
    uint32_t nlistener;
    uint32_t hash_power;
};

static struct listener_ht hashtable;
static struct listener_ht *ht = &hashtable;


static struct listener_slh *
_get_bucket(const struct buf_sock *s)
{
    /* use the _address_ of the channel to hash */
    uint32_t hval = hash_lookup3((char *)s, sizeof(struct buf_sock *), 0);
    return &(ht->table[hval & HASHMASK(ht->hash_power)]);
}

static void
_listener_reset(struct listener *l)
{
    l->s = NULL;
    l->ntopic = 0;
    TAILQ_INIT(l->idx);
}

static struct listener *
_listener_create(const struct buf_sock *s)
{
    struct listener *l;

    log_verb("create new listener for buf_sock %p", s);

    l = cc_alloc(sizeof(struct listener));
    if (l == NULL) {
        return NULL;
    }

    l->idx = cc_alloc(sizeof(struct index_tqh));
    if (l->idx == NULL) {
        cc_free(l);
        return NULL;
    }

    _listener_reset(l);
    l->s = (struct buf_sock *)s;

    return l;
}

static void
_listener_destroy(struct listener **l)
{
    ASSERT(l != NULL && *l != NULL);

    log_verb("destroy listener %p", *l);

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


struct listener *
listener_get(const struct buf_sock *s)
{
    struct listener_slh *bucket;
    struct listener *l;

    bucket = _get_bucket(s);
    for (l = SLIST_FIRST(bucket); l != NULL; l = SLIST_NEXT(l, l_sle)) {
        if (l->s == s) {
            return l;
        }
    }

    log_verb("listener not found for buf_sock", s);
    return NULL;
}

struct listener *
listener_add(const struct buf_sock *s)
{
    ASSERT(listener_get(s) == NULL);

    struct listener_slh *bucket;
    struct listener *l = _listener_create(s);

    log_verb("add listener %p for buf_sock %p", l, s);

    bucket = _get_bucket(s);
    SLIST_INSERT_HEAD(bucket, l, l_sle);

    ht->nlistener++;
    log_verb("total listeners: %"PRIu32, ht->nlistener);

    return l;
}

void
listener_delete(const struct buf_sock *s)
{
    struct listener_slh *bucket;
    struct listener *l, *prev;

    bucket = _get_bucket(s);
    for (prev = NULL, l = SLIST_FIRST(bucket); l != NULL;
        prev = l, l = SLIST_NEXT(l, l_sle)) {
        if (l == NULL) {
            log_debug("listener not found for buf_sock %p", s);
            return;
        }
        if (l->s == s) {
            break;
        }
    }

    if (prev == NULL) {
        SLIST_REMOVE_HEAD(bucket, l_sle);
    } else {
        SLIST_REMOVE_AFTER(prev, l_sle);
    }

    _listener_destroy(&l);
    --(ht->nlistener);
    log_verb("total listeners: %"PRIu32, ht->nlistener);
}

bool
listener_add_topic(struct listener *l, const struct topic *t)
{
    struct index_node *node;

    ASSERT(l != NULL && t != NULL);

    /* do nothing if already subscribed */
    TAILQ_FOREACH(node, l->idx, i_tqe) {
        if (node->obj == t) {
            log_debug("listener %p already subscribed to topic %p", l, t);
            return false;
        }
    }

    node = cc_alloc(sizeof(struct index_node));
    if (node == NULL) {
        log_error("cannot add topic: out of memory");
        return false;
    }
    node->obj = (struct topic *)t;

    TAILQ_INSERT_TAIL(l->idx, node, i_tqe);
    l->ntopic++;
    log_verb("listener %p subscribed to topic %p, total subscription: %"PRIu32,
            l, t, l->ntopic);

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
        log_debug("listener %p not subscribed to topic %p", l, t);
        return;
    }

    TAILQ_REMOVE(l->idx, node, i_tqe);
    l->ntopic--;
    cc_free(node);
    log_verb("listener %p unsubscribed from topic %p, total subscription: %"
            PRIu32, l, t, l->ntopic);
}

void
listener_setup(uint32_t hash_power)
{
    uint32_t i, nentry;
    struct listener_slh *table;

    ASSERT(hash_power > 0);

    ht->hash_power = hash_power;
    ht->nlistener = 0;
    nentry = HASHSIZE(ht->hash_power);

    table = cc_alloc(sizeof(*table) * nentry);
    if (table == NULL) {
        log_crit("listener setup failed: OOM");
        exit(EX_CONFIG);
    }
    ht->table = table;

    for (i = 0; i < nentry; ++i) {
        SLIST_INIT(&table[i]);
    }
}

void listener_teardown(void)
{
    if (ht->table != NULL) {
        cc_free(ht->table);
    }
}
