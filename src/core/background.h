#pragma once

/**
 * The background module is for managing the background and control plane thread.
 * This includes tasks such as logging, as well as admin port operations.
 */

#include <cc_define.h>
#include <cc_metric.h>

/*          name              type              default   description */
#define BACKGROUND_OPTION(ACTION)                                                     \
    ACTION( background_intvl, OPTION_TYPE_UINT, BG_INTVL, "background timer interval"    )\
    ACTION( admin_port,       OPTION_TYPE_STR,  "33333",  "admin port"                   )\
    ACTION( admin_host,       OPTION_TYPE_STR,  NULL,     "admin interfaces listening on")

#define BG_INTVL 100            /* 100 milliseconds */

struct addrinfo;

rstatus_t background_setup(struct addrinfo *ai, int tick);
void background_teardown(void);

void *background_evloop(void *arg);
