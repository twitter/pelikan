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

#define SIGNAL_MIN 1
#define SIGNAL_MAX 31

#ifndef sig_fn
typedef void (*sig_fn)(int);
#endif

struct signal {
    char *info;
    int flags;
    sig_fn handler;
    uint32_t mask;  /* additional singals to mask */
};

/**
 * to customize signal handling, users are suppose to overwrite entries in
 * signals after it is initialized.
 *
 * Note: the library has already overridden handlers for the following signals:
 * - SIGTTIN(debug): reload log file
 * - SIGSEGV(debug): print stacktrace before reraise segfault again
 * - SIGPIPE(channel): ignored, this prevents service from exiting when pipe closes
 */
struct signal signals[SIGNAL_MAX]; /* there are only 31 signals from 1 to 31 */

int signal_override(int signo, char *info, int flags, uint32_t mask, sig_fn handler);

#ifdef __cplusplus
}
#endif
