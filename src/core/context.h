#pragma once

/* this file is for internal use only by the core module */

#include <stdbool.h>

struct event_base;

struct context {
    struct event_base *evb;
    int timeout;
};

bool admin_init;
bool server_init;
bool worker_init;
bool debug_init;
