#pragma once

typedef enum compose_rstatus {
    COMPOSE_OK         = 0,
    COMPOSE_ENOMEM     = -1,
    COMPOSE_EOVERSIZED = -2,
} compose_rstatus_t;

struct buf;
struct request;
struct response;

/* if the return value is negative, it can be interpreted as compose_rstatus */
int compose_req(struct buf **buf, struct request *req);
int compose_rsp(struct buf **buf, struct response *rsp);
