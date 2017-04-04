#pragma once

#include "index.h"

#include <cc_bstring.h>

/*
 * a topic is an endpoint that clients can subscribe to, equivalent to
 * "channel" in the original redis protocol.
 */
struct topic {
    SLIST_ENTRY(topic)  t_sle;

    struct bstring      name;
    uint32_t            nsub;
    struct index_tqh    *idx; /* index of all listeners */
};

SLIST_HEAD(topic_slh, topic);

struct topic_ht {
    struct topic_slh *table;
    uint32_t ntopic;
    uint32_t hash_power;
};

struct listener;

struct topic_ht *topic_ht_create(uint32_t hash_power);
void topic_ht_destroy(struct topic_ht **ht);

struct topic *topic_ht_get(const struct bstring *name, struct topic_ht *ht);
void topic_ht_put(const struct topic *t, struct topic_ht *ht);
void topic_ht_delete(const struct bstring *name, struct topic_ht *ht);

struct topic *topic_create(const struct bstring *name);
void topic_destroy(struct topic **t);
void topic_reset(struct topic *t);

bool topic_add_listener(struct topic *t, const struct listener *l);
void topic_del_listener(struct topic *t, const struct listener *l);
