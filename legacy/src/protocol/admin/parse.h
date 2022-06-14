#pragma once

typedef enum parse_rstatus {
    PARSE_OK        = 0,
    PARSE_EUNFIN    = -1,
    PARSE_EINVALID  = -2,
    PARSE_EOTHER    = -3,
} parse_rstatus_e;

struct buf;
struct request;

parse_rstatus_e admin_parse_req(struct request *req, struct buf *buf);
