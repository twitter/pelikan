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

/*
Excerpt and modified from lookup3.c (http://burtleburtle.net/bob/c/lookup3.c),
originally by Bob Jenkins, May 2006, Public Domain.
*/

#include <cc_define.h>

#include <stdint.h>     /* defines uint32_t etc */
#include <stdlib.h>

uint32_t hashlittle( const void *key, size_t length, uint32_t initval);

#ifdef __cplusplus
}
#endif
