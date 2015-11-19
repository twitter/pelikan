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

#define DBUF_OPTION(ACTION)                                                                             \
    ACTION( dbuf_max_power,      OPTION_TYPE_UINT,   str(DBUF_DEFAULT_MAX), "max number of doubling"   )

#define DBUF_DEFAULT_MAX    6  /* with 16KiB default size, this gives us 1 MiB max */

/* Setup/teardown doubling buffer module */
void dbuf_setup(uint8_t power);
void dbuf_teardown(void);

/* Buffer resizing functions */
rstatus_i dbuf_double(struct buf **buf); /* 2x size, slightly >2x capacity */
rstatus_i dbuf_shrink(struct buf **buf); /* reset to initial size */
rstatus_i dbuf_fit(struct buf **buf, uint32_t cap); /* resize to fit cap */

#ifdef __cplusplus
}
#endif
