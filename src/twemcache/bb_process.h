#pragma once

#include <cc_define.h>

struct request;
struct buf;

rstatus_t process_request(struct request *req, struct buf *buf);
