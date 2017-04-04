#pragma once

#include <cc_queue.h>

struct index_node {
    TAILQ_ENTRY(index_node) i_tqe;
    void                    *obj;
};

TAILQ_HEAD(index_tqh, index_node);
