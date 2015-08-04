#pragma once

#include <cc_define.h>

#include <pthread.h>
#include <stdbool.h>

struct logger;
struct log_core;

/* Create a new thread that flushes logger every flush_interval usec */
struct log_core *log_core_create(struct logger *logger, int flush_interval);

/* Stop flushing the logger (stops the flushing thread) */
void log_core_destroy(struct log_core **core);
