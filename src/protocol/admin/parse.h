#pragma once

typedef enum parse_rstatus {
    PARSE_OK        = 0,
    PARSE_EUNFIN    = -1,
    PARSE_EINVALID  = -2,
    PARSE_EOTHER    = -3,
} parse_rstatus_t;

struct buf;
struct op;
struct reply;

parse_rstatus_t parse_op(struct op *op, struct buf *buf);
