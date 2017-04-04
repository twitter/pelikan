#pragma once

#include "index.h"

#include <channel/cc_channel.h>

/*
 * a listener is a client that has subscribed to at least one channel
 */
struct listener {
    SLIST_ENTRY(listener)   l_sle;

    channel_p               ch;
    channel_handler_st      *handler;
    uint32_t                ntopic;
    struct index_tqh        *idx; /* index of all topics */
};

SLIST_HEAD(listener_slh, listener);

struct listener_ht {
    struct listener_slh *table;
    uint32_t nlistener;
    uint32_t hash_power;
};

struct topic;

struct listener_ht *listener_ht_create(uint32_t hash_power);
void listener_ht_destroy(struct listener_ht **ht);

struct listener *listener_ht_get(const channel_p ch, struct listener_ht *ht);
void listener_ht_put(const struct listener *l, struct listener_ht *ht);
void listener_ht_delete(const channel_p ch, struct listener_ht *ht);

struct listener *listener_create(channel_p ch, channel_handler_st *handler);
void listener_destroy(struct listener **l);
void listener_reset(struct listener *l);

bool listener_add_topic(struct listener *l, const struct topic *t);
void listener_del_topic(struct listener *l, const struct topic *t);
