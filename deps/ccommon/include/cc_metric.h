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

/**
 * Note: there's no gcc built-in atomic primitives to do a straight-up store
 * atomically. But so far we only use the UPDATE_* macros for sys metrics, so
 * it doesn't matter much.
 * We can also use an extra variable to store the current value and use a CAS
 * primitive with the value read as well as the value to set, but the extra
 * variable is a headache.
 * Will revisit this later.
 */
#define metric_update_val(_metric, _val) do {                               \
    if ((_metric).type == METRIC_COUNTER) {                                 \
         (_metric).counter = (uint64_t)_val;                                \
    } else if ((_metric).type == METRIC_GAUGE) {                            \
         (_metric).gauge = (int64_t)_val;                                   \
    } else if ((_metric).type == METRIC_FPN) {                              \
         (_metric).fpn = (double)_val;                                      \
    } else { /* error  */                                                   \
    }                                                                       \
} while(0)

#define UPDATE_VAL(_base, _metric, _val) do {                               \
    if ((_base) != NULL) {                                                  \
         metric_update_val((_base)->_metric, _val);                         \
    }                                                                       \
} while(0)


#define METRIC_DECLARE(_name, _type, _description)   \
    struct metric _name;

#define METRIC_INIT(_name, _type, _description)      \
    ._name = {.name = #_name, .desc = _description, .type = _type},

#define METRIC_NAME(_name, _type, _description)      \
    #_name,

#else

#define INCR(_base, _metric)
#define INCR_N(_base, _metric, _delta)
#define DECR(_base, _metric)
#define DECR_N(_base, _metric, _delta)
#define UPDATE_VAL(_base, _metric, _val)

#define METRIC_DECLARE(_name, _type, _description)
#define METRIC_INIT(_name, _type, _description)
#define METRIC_NAME(_name, _type, _description)

#endif

#define METRIC_CARDINALITY(_o) sizeof(_o) / sizeof(struct metric)

typedef enum metric_type {
    METRIC_COUNTER, /* supports INCR/INCR_N/UPDATE_VAL */
    METRIC_GAUGE,   /* supports INCR/INCR_N/DECR/DECR_N/UPDATE_VAL */
    METRIC_FPN      /* supports UPDATE_VAL */
} metric_type_e;

extern char *metric_type_str[3];

/* Note: anonymous union does not work with older (<gcc4.7) compilers */
/* TODO(yao): determine if we should dynamically allocate the value field
 * during init. The benefit is we don't have to allocate the same amount of
 * memory for different types of values, potentially wasting space. */
struct metric {
    char *name;
    char *desc;
    metric_type_e type;
    union {
        uint64_t    counter;
        int64_t     gauge;
        double      fpn;
    };
};

void metric_reset(struct metric sarr[], unsigned int nmetric);
size_t metric_print(char *buf, size_t nbuf, char *fmt, struct metric *m);
size_t metric_describe(char *buf, size_t nbuf, char *fmt, struct metric *m);

#ifdef __cplusplus
}
#endif
