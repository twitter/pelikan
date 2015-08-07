/*
 * ccommon - a cache common library.
 * Copyright (C) 2015 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/*
 * rbuf: a ring buffer array designed for logging use (NOT THREADSAFE!)
 */

#pragma once

#include <cc_metric.h>

#include <stdint.h>
#include <unistd.h>

struct rbuf;

/*          name            type            description */
#define RBUF_METRIC(ACTION)                                           \
    ACTION( rbuf_create,    METRIC_COUNTER, "# rbuf created"         )\
    ACTION( rbuf_create_ex, METRIC_COUNTER, "# rbuf create errors"   )\
    ACTION( rbuf_destroy,   METRIC_COUNTER, "# rbuf destroyed"       )\
    ACTION( rbuf_curr,      METRIC_GAUGE,   "# rbuf allocated"       )\
    ACTION( rbuf_byte,      METRIC_GAUGE,   "# rbuf bytes allocated" )

typedef struct {
    RBUF_METRIC(METRIC_DECLARE)
} rbuf_metrics_st;

#define RBUF_METRIC_INIT(_metrics) do {                           \
    *(_metrics) = (rbuf_metrics_st) { RBUF_METRIC(METRIC_INIT) }; \
} while(0)

/* setup/teardown */
void rbuf_setup(rbuf_metrics_st *metrics);
void rbuf_teardown(void);

/* creation/destruction */
struct rbuf *rbuf_create(uint32_t cap);
void rbuf_destroy(struct rbuf *buf);

/* read/write capacity */
size_t rbuf_rcap(struct rbuf *buf);
size_t rbuf_wcap(struct rbuf *buf);

/* read from rbuf into a buffer in memory */
size_t rbuf_read(void *dst, struct rbuf *src, size_t n);
/* write from a buffer in memory to the rbuf */
size_t rbuf_write(struct rbuf *dst, void *src, size_t n);

/* read from rbuf to the fd. attempts to empty the buffer. */
ssize_t rbuf_read_fd(struct rbuf *src, int fd);;
