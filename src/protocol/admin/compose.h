#pragma once

typedef enum compose_rstatus {
    COMPOSE_OK         = 0,
    COMPOSE_ENOMEM     = -1,
    COMPOSE_EOVERSIZED = -2,
} compose_rstatus_e;

struct buf;
struct request;
struct response;

/* if the return value is negative, it can be interpreted as compose_rstatus */
int admin_compose_req(struct buf **buf, const struct request *req);
int admin_compose_rsp(struct buf **buf, const struct response *rsp);
