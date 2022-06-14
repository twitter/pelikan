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

#include <buffer/cc_buf.h>
#include <cc_define.h>
#include <cc_util.h>

#include <stdbool.h>


/*          name                type                default             description */
#define DBUF_OPTION(ACTION)                                                                     \
    ACTION( dbuf_max_power,      OPTION_TYPE_UINT,  DBUF_DEFAULT_MAX,   "max number of doubles")

typedef struct {
    DBUF_OPTION(OPTION_DECLARE)
} dbuf_options_st;

#define DBUF_DEFAULT_MAX    8  /* with 16KiB default size, this gives us 4 MiB max */

/*          name            type            description */
#define DBUF_METRIC(ACTION)                                         \
    ACTION( dbuf_double,    METRIC_COUNTER, "# double completed"   )\
    ACTION( dbuf_double_ex, METRIC_COUNTER, "# double failed"      )\
    ACTION( dbuf_shrink,    METRIC_COUNTER, "# shrink completed"   )\
    ACTION( dbuf_shrink_ex, METRIC_COUNTER, "# shrink failed"      )\
    ACTION( dbuf_fit,       METRIC_COUNTER, "# fit completed"      )\
    ACTION( dbuf_fit_ex,    METRIC_COUNTER, "# fit failed"         )

typedef struct {
    DBUF_METRIC(METRIC_DECLARE)
} dbuf_metrics_st;

/* Setup/teardown doubling buffer module */
void dbuf_setup(dbuf_options_st *options, dbuf_metrics_st *metrics);
void dbuf_teardown(void);

/* Buffer resizing functions */
rstatus_i dbuf_double(struct buf **buf); /* 2x size, slightly >2x capacity */
/* shrink to initial size or content size, whichever is larger */
rstatus_i dbuf_shrink(struct buf **buf);
rstatus_i dbuf_fit(struct buf **buf, uint32_t cap); /* resize to fit cap */

#ifdef __cplusplus
}
#endif
