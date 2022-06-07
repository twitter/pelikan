#pragma once

/* this file is for internal use only by the core module */

#include <stdbool.h>

struct event_base;

struct context {
    struct event_base *evb;
    int timeout;
};

extern bool admin_init;
extern bool server_init;
extern bool worker_init;
