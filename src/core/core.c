#include "core.h"

#include "context.h"

#include <cc_debug.h>

#include <errno.h>
#include <pthread.h>
#include <string.h>
#include <sysexits.h>

void
worker_run(void *arg_worker)
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

void
pubsub_run(void *arg_pubsub)
{
    pthread_t pubsub, server;
    int ret;

    if (!admin_init || !server_init || !pubsub_init) {
        log_crit("cannot run: admin/server/pubsub have to be initialized");
        return;
    }

    ret = pthread_create(&pubsub, NULL, core_pubsub_evloop, arg_pubsub);
    if (ret != 0) {
        log_crit("pthread create failed for pubsub thread: %s", strerror(ret));
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
