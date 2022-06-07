#include "core.h"

#include "context.h"

#include <cc_debug.h>

#include <errno.h>
#include <pthread.h>
#include <string.h>
#include <sysexits.h>

bool admin_init;
bool server_init;
bool worker_init;

void
core_run(void *arg_worker)
{
    pthread_t worker, server;
    int ret;

    if (!admin_init || !server_init || !worker_init) {
        log_crit("cannot run: admin/server/worker have to be initialized");
        return;
    }

    ret = pthread_create(&worker, NULL, core_worker_evloop, arg_worker);
    if (ret != 0) {
        log_crit("pthread create failed for worker thread: %s", strerror(ret));
        goto error;
    }

    ret = pthread_create(&server, NULL, core_server_evloop, NULL);
    if (ret != 0) {
        log_crit("pthread create failed for server thread: %s", strerror(ret));
        goto error;
    }

    core_admin_evloop();

error:
    exit(EX_OSERR);
}
