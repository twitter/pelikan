#include "core.h"

#include "context.h"

#include <cc_debug.h>

#include <errno.h>
#include <pthread.h>
#include <string.h>
#include <sysexits.h>

pthread_t worker, server;
void
core_run(void *arg_worker, void *arg_server)
{
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

    ret = pthread_create(&server, NULL, core_server_evloop, arg_server);
    if (ret != 0) {
        log_crit("pthread create failed for server thread: %s", strerror(ret));
        goto error;
    }

    core_admin_evloop();

error:
    exit(EX_OSERR);
}

void core_destroy(void)
{
    int ret;

    if (!server_init || !worker_init) {
        log_crit("cannot run: server/worker have to be initialized");
        return;
    }

    ret = pthread_join(server, NULL);
    if (ret != 0) {
        log_crit("pthread join failed for worker thread: %s", strerror(ret));
        exit(EX_OSERR);
    }

    ret = pthread_join(worker, NULL);
    if (ret != 0) {
        log_crit("pthread join failed for server thread: %s", strerror(ret));
        exit(EX_OSERR);
    }
}
