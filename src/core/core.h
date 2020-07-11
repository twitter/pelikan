#pragma once

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include "admin/admin.h"
#include "data/shared.h"
#include "data/server.h"
#include "data/worker.h"

void core_run(void *arg_worker, void *arg_server);
void core_destroy(void);
