#pragma once

typedef enum compose_rstatus {
    COMPOSE_OK         = 0,
    COMPOSE_ENOMEM     = -1,
    COMPOSE_EOVERSIZED = -2,
} compose_rstatus_t;

struct buf;
struct op;
struct reply;

/* if the return value is negative, it can be interpreted as compose_rstatus */
int compose_op(struct buf **buf, struct op *op);
int compose_rep(struct buf **buf, struct reply *rep);
