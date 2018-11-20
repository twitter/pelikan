#pragma once

/**
 * The debug thread is for performing potentially expensive investigative tasks.
 * User should avoid concurrent access to this port/thread.
 */

#include <cc_define.h>
#include <cc_option.h>

#include <stdbool.h>

#define DEBUG_HOST      NULL
#define DEBUG_PORT      "9900"
#define DEBUG_TIMEOUT   100     /* in ms */
#define DEBUG_NEVENT    1

/*          name            type                default         description */
#define DEBUG_OPTION(ACTION)                                                                    \
    ACTION( debug_host,     OPTION_TYPE_STR,    DEBUG_HOST,     "debug interfaces listening on")\
    ACTION( debug_port,     OPTION_TYPE_STR,    DEBUG_PORT,     "debug port"                   )\
    ACTION( debug_timeout,  OPTION_TYPE_UINT,   DEBUG_TIMEOUT,  "evwait timeout"               )\
    ACTION( debug_nevent,   OPTION_TYPE_UINT,   DEBUG_NEVENT,   "evwait max nevent returned"   )

typedef struct {
    DEBUG_OPTION(OPTION_DECLARE)
} debug_options_st;

void core_debug_setup(debug_options_st *options);
void core_debug_teardown(void);

void core_debug_evloop(void);
