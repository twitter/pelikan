#pragma once

struct pipe_conn;
struct ring_array;

/* pipe for server/worker thread communication */
#ifdef USE_EVENT_FD
extern int event_fd_s2w;
#else
extern struct pipe_conn *pipe_new;
#endif
extern struct pipe_conn *pipe_term;

/* array holding accepted connections */
extern struct ring_array *conn_new;
extern struct ring_array *conn_term;
