#pragma once

struct event_base;
struct ring_array;

struct context {
    struct event_base *evb;
    int timeout;
};

/* pipe for signaling connections */
extern int conn_fds[2];

/* array holding accepted connections */
extern struct ring_array *conn_arr;
