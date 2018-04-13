#pragma once

#include "index.h"

#include <stream/cc_sockio.h>

/*
 * a listener is a client that has subscribed to at least one channel
 */
struct listener {
    SLIST_ENTRY(listener)   l_sle;

    struct buf_sock         *s;
    uint32_t                ntopic;
    struct index_tqh        *idx; /* index of all topics */
};

struct topic;

void listener_setup(uint32_t hash_power);
void listener_teardown(void);

struct listener *listener_get(const struct buf_sock *s);
struct listener *listener_add(const struct buf_sock *s);
void listener_delete(const struct buf_sock *s);

bool listener_add_topic(struct listener *l, const struct topic *t);
void listener_del_topic(struct listener *l, const struct topic *t);
