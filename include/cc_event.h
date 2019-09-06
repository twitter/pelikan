/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
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

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_define.h>
#include <cc_metric.h>

#include <inttypes.h>

#define EVENT_READ  0x0000ff
#define EVENT_WRITE 0x00ff00
#define EVENT_ERR   0xff0000

/*          name                type            description */
#define EVENT_METRIC(ACTION)                                            \
    ACTION( event_total,        METRIC_COUNTER, "# events returned"    )\
    ACTION( event_loop,         METRIC_COUNTER, "# event loop returns" )\
    ACTION( event_read,         METRIC_COUNTER, "# reads registered"   )\
    ACTION( event_write,        METRIC_COUNTER, "# writes registered"  )

typedef struct {
    EVENT_METRIC(METRIC_DECLARE)
} event_metrics_st;

typedef void (*event_cb_fn)(void *, uint32_t);  /* event callback */

struct event_base;

void event_setup(event_metrics_st *metrics);
void event_teardown(void);

/* event base */
struct event_base *event_base_create(int nevent, event_cb_fn cb);
void event_base_destroy(struct event_base **evb);

/* event control */
/**
 * Note: the asymmetry between add and del is intentional:
 * - when enabling events on a fd, different types correspond to different
 *   follow-up actions, and are often added independently. For example, a
 *   server socket does not ever need to write to its sd, and therefore will
 *   never use write events.
 * - when removing events from a fd, it is common to delete both types as part
 *   of the teardown routine. So it is convenient to provide an API to clean
 *   up whatever flag that was set.
 */
int event_add_read(struct event_base *evb, int fd, void *data);
int event_add_write(struct event_base *evb, int fd, void *data);
int event_del(struct event_base *evb, int fd);

/* event wait */
int event_wait(struct event_base *evb, int timeout);

#ifdef __cplusplus
}
#endif
