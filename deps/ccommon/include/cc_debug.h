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
#include <cc_log.h>

#include <stdint.h>

/*
 * Wrappers for defining custom assert based on whether macro
 * CC_ASSERT_PANIC or CC_ASSERT_LOG was defined at the moment
 * ASSERT was called.
 */
#if defined CC_ASSERT_PANIC && CC_ASSERT_PANIC == 1 /* log and panic */

#define ASSERT(_x) do {                             \
    if (!(_x)) {                                    \
        debug_assert(#_x, __FILE__, __LINE__, 1);   \
    }                                               \
} while (0)

#define NOT_REACHED() ASSERT(0)

#elif defined CC_ASSERT_LOG && CC_ASSERT_LOG == 1 /* just log */

#define ASSERT(_x) do {                             \
    if (!(_x)) {                                    \
        debug_assert(#_x, __FILE__, __LINE__, 0);   \
    }                                               \
} while (0)

#define NOT_REACHED() ASSERT(0)

#else /* ignore all asserts */

#define ASSERT(_x)

#define NOT_REACHED()

#endif

void debug_assert(const char *cond, const char *file, int line, int panic);
void debug_stacktrace(int skip_count);

rstatus_t debug_setup(int level, char *filename, uint32_t buf_cap);
void debug_teardown(void);

/**
 **********************************************
 * Debug logging
 **********************************************
 */

#define LOG_DEBUG_LEVEL 4         /* default log level */
#define LOG_DEBUG_NBUF  16 * KiB  /* default log buf size */
#define LOG_DEBUG_INTVL 100000    /* flush every 100 milliseconds */

/*          name             type              default               description */
#define LOG_DEBUG_OPTION(ACTION) \
    ACTION( log_debug_level, OPTION_TYPE_UINT, str(LOG_DEBUG_LEVEL), "debug log level"          )\
    ACTION( log_debug_file,  OPTION_TYPE_STR,  NULL,                 "debug log file"           )\
    ACTION( log_debug_nbuf,  OPTION_TYPE_UINT, str(LOG_DEBUG_NBUF),  "debug log buf size"       )\
    ACTION( log_debug_intvl, OPTION_TYPE_UINT, str(LOG_DEBUG_INTVL), "debug log flush interval" )

extern struct logger *debug_logger;

/*
 * TODO(yao): a reasonable guideline for using these different levels.
 */
#define LOG_ALWAYS  0   /* always log, special value  */
#define LOG_CRIT    1   /* critical: usually warrants exiting */
#define LOG_ERROR   2   /* error: may need action */
#define LOG_WARN    3   /* warning: may need attention */
#define LOG_INFO    4   /* informational: important but normal */
#define LOG_DEBUG   5   /* debug: abnormal behavior that's not an error */
#define LOG_VERB    6   /* verbose: showing normal logic flow */
#define LOG_VVERB   7   /* verbose on crack, for annoying msg e.g. timer */

/* NOTE(yao): it may be useful to have a sampled log func for bursty events */

/*
 * log_stderr   - log to stderr
 *
 * loga         - log always
 * loga_hexdump - log hexdump always
 *
 * log_panic    - log messages followed by a panic, when LOG_CRIT is met
 * log_error    - error log messages
 * log_warn     - warning log messages
 * ...
 *
 * log          - debug log messages based on a log level (subject to config)
 * log_hexdump  - hexadump -C of a log buffer (subject to config)
 */

#define loga(...) do {                                                      \
    _log(debug_logger, __FILE__, __LINE__, LOG_ALWAYS, __VA_ARGS__);        \
} while (0)

#define loga_hexdump(_data, _datalen, ...) do {                             \
    _log(debug_logger, __FILE__,__LINE__, LOG_ALWAYS, __VA_ARGS__);         \
    _log_hexdump(debug_logger, -1, (char *)(_data), (int)(_datalen));       \
} while (0)

#define log_panic(...) do {                                                 \
    _log(debug_logger, __FILE__, __LINE__, LOG_CRIT, __VA_ARGS__);          \
    abort();                                                                \
} while (0)

#if defined CC_LOGGING && CC_LOGGING == 1

#define log_crit(...) do {                                                  \
    _log(debug_logger, __FILE__, __LINE__, LOG_CRIT, __VA_ARGS__);          \
} while (0)

#define log_error(...) do {                                                 \
    _log(debug_logger, __FILE__, __LINE__, LOG_ERROR, __VA_ARGS__);         \
} while (0)

#define log_warn(...) do {                                                  \
    _log(debug_logger, __FILE__, __LINE__, LOG_WARN, __VA_ARGS__);          \
} while (0)

#define log_info(...) do {                                                  \
    _log(debug_logger, __FILE__, __LINE__, LOG_INFO, __VA_ARGS__);          \
} while (0)

#define log_debug(...) do {                                                 \
    _log(debug_logger, __FILE__, __LINE__, LOG_DEBUG, __VA_ARGS__);         \
} while (0)

#define log_verb(...) do {                                                  \
    _log(debug_logger, __FILE__, __LINE__, LOG_VERB, __VA_ARGS__);          \
} while (0)

#define log_vverb(...) do {                                                 \
    _log(debug_logger, __FILE__, __LINE__, LOG_VVERB, __VA_ARGS__);         \
} while (0)

#define log(_level, ...) do {                                               \
    _log(debug_logger, __FILE__, __LINE__, _level, __VA_ARGS__);            \
} while (0)

#define log_hexdump(_level, _data, _datalen, ...) do {                      \
    _log(debug_logger, __FILE__,__LINE__, _level, __VA_ARGS__);             \
    _log_hexdump(debug_logger, _level, (char *)(_data), (int)(_datalen));   \
} while (0)

#else

#define log_crit(...)
#define log_error(...)
#define log_warn(...)
#define log_info(...)
#define log_debug(...)
#define log_verb(...)
#define log_vverb(...)

#define log(_level, ...)
#define log_hexdump(_level, _data, _datalen, ...)

#endif

void _log(struct logger *logger, const char *file, int line, int level, const char *fmt, ...);

#ifdef __cplusplus
}
#endif
