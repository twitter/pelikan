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

#include <config.h>

#ifdef HAVE_SIGNAME
# define CC_HAVE_SIGNAME 1
#endif

#ifdef HAVE_STATS
# define CC_STATS 1
#endif

#ifdef HAVE_LOGGING
# define CC_LOGGING 1
#endif

#ifdef HAVE_ASSERT_PANIC
# define CC_ASSERT_PANIC 1
#endif

#ifdef HAVE_ASSERT_LOG
# define CC_ASSERT_LOG 1
#endif

#ifdef HAVE_BACKTRACE
# define CC_BACKTRACE 1
#endif

#ifdef HAVE_ACCEPT4
# define CC_ACCEPT4 1
#endif

#ifdef HAVE_DEBUG_MM
#define CC_DEBUG_MM 1
#endif

#define CC_OK        0
#define CC_ERROR    -1

#define CC_EAGAIN   -2
#define CC_ERETRY   -3

#define CC_ENOMEM   -4
#define CC_EEMPTY   -5 /* no data */

#define CC_ERDHUP   -6 /* EOF/peer closed */
#define CC_EINVAL   -7

#define CC_UNFIN    1  /* unfinished, more data expected */

typedef int rstatus_i;  /* generic function return value type */
typedef int err_i; /* erroneous values for rstatus_i */

#ifdef __cplusplus
}
#endif
