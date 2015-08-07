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

#include <stdbool.h>
#include <stdint.h>

struct timer {
    bool started;
    bool stopped;
    uint64_t start;
    uint64_t stop;
};

/* update timer */
void timer_reset(struct timer *t);
void timer_start(struct timer *t);
void timer_stop(struct timer *t);

/* read timer */
double timer_duration_ns(struct timer *t);
double timer_duration_us(struct timer *t);
double timer_duration_ms(struct timer *t);
double timer_duration_sec(struct timer *t);

#ifdef __cplusplus
}
#endif
