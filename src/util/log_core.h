#pragma once

#include <cc_define.h>

struct logger;

/* Create a new thread that flushes logger every flush_interval usec */
rstatus_t log_core_create(struct logger *logger, int flush_interval);
