#pragma once

#include <stream/cc_sockio.h>

int pingserver_process_read(struct buf_sock *s);
int pingserver_process_write(struct buf_sock *s);
