#pragma once

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include "admin/admin.h"
#include "context.h"
#include "data/server.h"
#include "data/worker.h"

#include <cc_define.h>

void core_setup(admin_options_st *opt_admin,
                server_options_st *opt_server, worker_options_st *opt_worker,
                server_metrics_st *smetrics, worker_metrics_st *wmetrics);
void core_teardown(void);
void core_run(void *arg_worker);
