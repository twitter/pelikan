#pragma once

typedef enum parse_rstatus {
    PARSE_OK        = 0,
    PARSE_EUNFIN    = -1,
    PARSE_EINVALID  = -2,
    PARSE_EOTHER    = -3,
} parse_rstatus_e;

struct buf;
struct request;

/* admin parser blocks "debug" commands which can block for a long time */
parse_rstatus_e admin_parse_req(struct request *req, struct buf *buf);
parse_rstatus_e debug_parse_req(struct request *req, struct buf *buf);
