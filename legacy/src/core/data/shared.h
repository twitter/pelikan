#pragma once

struct pipe_conn;
struct ring_array;

/* pipe for server/worker thread communication */
#ifdef USE_EVENT_FD
extern int efd_server_to_worker;
extern int efd_worker_to_server;
#else
extern struct pipe_conn *pipe_new;
extern struct pipe_conn *pipe_term;
#endif

/* array holding accepted connections */
extern struct ring_array *conn_new;
extern struct ring_array *conn_term;
