#pragma once

struct event_base;

struct context {
    struct event_base *evb;
    int timeout;
};
