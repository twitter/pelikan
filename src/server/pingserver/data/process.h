#pragma once

#include <buffer/cc_buf.h>

int pingserver_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int pingserver_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
