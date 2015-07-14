#pragma once

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include <core/server.h>
#include <core/worker.h>

#include <cc_define.h>
#include <channel/cc_channel.h>

struct addrinfo;

rstatus_t core_setup(struct addrinfo *ai, uint32_t max_conns, server_metrics_st *smetrics, worker_metrics_st *wmetrics);
void core_teardown(void);
void core_run(void);
