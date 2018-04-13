#pragma once

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include "admin/admin.h"
#include "data/pubsub.h"
#include "data/shared.h"
#include "data/server.h"
#include "data/worker.h"

void worker_run(void *arg_worker);
void pubsub_run(void *arg_pubsub);
