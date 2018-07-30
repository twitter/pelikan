/*
 * ccommon - a cache common library.
 * Copyright (C) 2018 Twitter, Inc.
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

/* recursively remove all content under path then unlinks path.
 * returns 0 on success, -1 on failure and sets errno.
*/
int
cc_util_rm_rf_rs(const char *path);


#ifdef __cplusplus
}
#endif
