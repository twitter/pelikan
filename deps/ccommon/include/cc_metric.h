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

#include <inttypes.h>
#include <stddef.h>

#if defined CC_STATS && CC_STATS == 1

#define metric_incr_n(_metric, _delta) do {                                 \
    if ((_metric).type == METRIC_COUNTER) {                                 \
         __atomic_add_fetch(&(_metric).counter, (_delta), __ATOMIC_RELAXED);\
    } else if ((_metric).type == METRIC_GAUGE) {                            \
         __atomic_add_fetch(&(_metric).gauge, (_delta), __ATOMIC_RELAXED);  \
    } else { /* error  */                                                   \
    }                                                                       \
} while(0)
#define metric_incr(_metric) metric_incr_n(_metric, 1)

#define INCR_N(_base, _metric, _delta) do {                                 \
    if ((_base) != NULL) {                                                  \
         metric_incr_n((_base)->_metric, _delta);                           \
    }                                                                       \
} while(0)
#define INCR(_base, _metric) INCR_N(_base, _metric, 1)

#define metric_decr_n(_metric, _delta) do {                                 \
    if ((_metric).type == METRIC_GAUGE) {                                   \
         __atomic_sub_fetch(&(_metric).gauge, (_delta), __ATOMIC_RELAXED);  \
    } else { /* error  */                                                   \
    }                                                                       \
} while(0)
#define metric_decr(_metric) metric_decr_n(_metric, 1)

#define DECR_N(_base, _metric, _delta) do {                                 \
    if ((_base) != NULL) {                                                  \
         metric_decr_n((_base)->_metric, _delta);                           \
    }                                                                       \
} while(0)
#define DECR(_base, _metric) DECR_N(_base, _metric, 1)


#define METRIC_DECLARE(_name, _type, _description)   \
    struct metric _name;

#define METRIC_INIT(_name, _type, _description)      \
    ._name = {.name = #_name, .type = _type},

#define METRIC_NAME(_name, _type, _description)      \
    #_name,

#else

#define metric_incr(_metric)
#define metric_incr_n(_metric, _delta)
#define metric_decr(_metric)
#define metric_decr_n(_metric, _delta)

#define METRIC_DECLARE(_name, _type, _description)
#define METRIC_INIT(_name, _type, _description)
#define METRIC_NAME(_name, _type, _description)

#endif

#define METRIC_CARDINALITY(_o) sizeof(_o) / sizeof(struct metric)

typedef enum metric_type {
    METRIC_COUNTER,
    METRIC_GAUGE,
    /* directly set values */
    METRIC_DDOUBLE,
    METRIC_DINTMAX
} metric_type_t;

typedef uint64_t counter_t;
typedef int64_t gauge_t;

/* Note: anonymous union does not work with older (<gcc4.7) compilers */
/* TODO(yao): determine if we should dynamically allocate the value field
 * during init. The benefit is we don't have to allocate the same amount of
 * memory for different types of values, potentially wasting space. */
struct metric {
    char *name;
    metric_type_t type;
    union {
        counter_t counter;
        gauge_t gauge;
        double vdouble;
        intmax_t vintmax;
    };
};

void metric_reset(struct metric sarr[], unsigned int nmetric);
void metric_setup(void);
void metric_teardown(void);

#ifdef __cplusplus
}
#endif
