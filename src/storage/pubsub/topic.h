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

struct listener;

void topic_setup(uint32_t hash_power);
void topic_teardown(void);

struct topic *topic_get(const struct bstring *name);
struct topic *topic_add(const struct bstring *name);
void topic_delete(const struct bstring *name);

bool topic_add_listener(struct topic *t, const struct listener *l);
void topic_del_listener(struct topic *t, const struct listener *l);
