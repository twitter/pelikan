#pragma once

/**
 * The background module is for managing the background and control plane thread.
 * This includes tasks such as logging, as well as admin port operations.
 */

#include <cc_define.h>
#include <cc_metric.h>

#include <stdbool.h>

/*          name            type                default         description */
#define ADMIN_OPTION(ACTION)                                                                    \
    ACTION( admin_intvl,    OPTION_TYPE_UINT,   MAINT_INTVL,    "maintenance timer interval"   )\
    ACTION( admin_port,     OPTION_TYPE_STR,    "33333",        "admin port"                   )\
    ACTION( admin_host,     OPTION_TYPE_STR,    NULL,           "admin interfaces listening on")\
    ACTION( admin_tw_tick,  OPTION_TYPE_UINT,   ADMIN_TW_TICK,  "timing wheel granularity (ns)")\
    ACTION( admin_tw_cap,   OPTION_TYPE_UINT,   ADMIN_TW_CAP,   "# ticks in timing wheel"      )\
    ACTION( admin_tw_ntick, OPTION_TYPE_UINT,   ADMIN_TW_NTICK, "max # ticks processed per ex" )

#define MAINT_INTVL    100            /* 100 milliseconds */
#define ADMIN_TW_TICK  1000000        /* 1000000 ns, or 1 millisecond */
#define ADMIN_TW_CAP   1000           /* 1000 ticks in timing wheel */
#define ADMIN_TW_NTICK 1000           /* max # ticks to process per exec */

struct addrinfo;
struct timing_wheel;
struct timeout_event;

extern struct timing_wheel *tw;
extern bool admin_running;

rstatus_i core_admin_setup(struct addrinfo *ai, int intvl, uint64_t tw_tick_ns,
                      size_t tw_cap, size_t tw_ntick);
void core_admin_teardown(void);

/* timeout events must be added while admin evloop is not running */
rstatus_i core_admin_add_tev(struct timeout_event *tev);

void *core_admin_evloop(void *arg);
