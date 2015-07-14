#pragma once

struct event_base;
struct pipe_conn;
struct ring_array;

struct context {
    struct event_base *evb;
    int timeout;
};

/* pipe for server/worker thread communication */
extern struct pipe_conn *pipe_c;

/* array holding accepted connections */
extern struct ring_array *conn_arr;
