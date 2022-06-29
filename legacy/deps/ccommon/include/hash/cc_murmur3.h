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

/**
 * The cc_murmur3.[ch] are adapted from the canonical implementation of
 * MurmurHash3 by Austin Appleby, released as part of SMHasher:
 *   https://github.com/aappleby/smhasher
 *
 * Changes include renaming functions, removing MSVC-related code, adding "static"
 * keyword to local-scope functions according to C language spec (original code is
 * in C++), to better fit them into the scope and style of ccommon
 *
 * The actual implementation is untouched.
 */

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>


void hash_murmur3_32(const void *key, int len, uint32_t seed, void *out);

void hash_murmur3_128_x86(const void *key, int len, uint32_t seed, void *out);

void hash_murmur3_128_x64(const void *key, int len, uint32_t seed, void *out);

#ifdef __cplusplus
}
#endif
