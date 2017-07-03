#pragma once

struct pipe_conn;
struct ring_array;

/* pipe for server/worker thread communication */
extern struct pipe_conn *pipe_c;

/* array holding accepted connections */
extern struct ring_array *conn_arr;

void core_shared_setup(void);
void core_shared_teardown(void);
