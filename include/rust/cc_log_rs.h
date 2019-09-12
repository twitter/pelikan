/* ccommon - a cache common library.
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

#include <cc_log.h>
#include <cc_bstring.h>

/* NOTE: for documentation see ccommon/rust/ccommon_rs/src/log.rs */

typedef enum log_level_rs {
    LOG_LEVEL_ERROR = 1,
    LOG_LEVEL_WARN,
    LOG_LEVEL_INFO,
    LOG_LEVEL_DEBUG,
    LOG_LEVEL_TRACE,
} log_level_rs_e;


typedef enum log_status_rs {
    /* Good work! */
    LOG_STATUS_OK = 0,
    /* An action that requires log_rs_is_setup() to be true, but it isn't
     * i.e. you need to call log_rs_setup() before whatever you just tried to do. */
    LOG_STATUS_NOT_SETUP_ERROR,
    /* We could not register as the backend for the log crate .
     * This state is unrecoverable. */
    LOG_STATUS_REGISTRATION_FAIL,
    /* Returned when there is already a logger set up for rust. */
    LOG_STATUS_ALREADY_SET_ERROR,
    /* Data was expected to be valid UTF8 but was not */
    LOG_STATUS_INVALID_UTF8,
    /* Failed to create a logger instance */
    LOG_STATUS_CREATION_ERROR,
    /* An unexpected error occurred, check stderr */
    LOG_STATUS_OTHER_FAILURE,
    /* You suck at programming */
    LOG_STATUS_NULL_POINTER_ERROR,
} log_status_rs_e;

struct log_config_rs {
    uint32_t buf_size;
    log_level_rs_e level;
    struct bstring path;
    struct bstring prefix;
};

struct log_handle_rs;

struct log_handle_rs* log_create_handle_rs(struct log_config_rs *cfg);
log_status_rs_e log_shutdown_rs(struct log_handle_rs *handle, uint32_t timeout_ms);
void log_destroy_handle_rs(struct log_handle_rs **h);

bool log_is_setup_rs(struct log_handle_rs *handle);

#ifdef __cplusplus
}
#endif
