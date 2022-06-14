#pragma once

#include <stream/cc_sockio.h>

int pingserver_process_read(struct buf **rbuf, struct buf **wbuf, void **data);
int pingserver_process_write(struct buf **rbuf, struct buf **wbuf, void **data);
int pingserver_process_error(struct buf **rbuf, struct buf **wbuf, void **data);
