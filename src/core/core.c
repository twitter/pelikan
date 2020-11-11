#include "core.h"

#include "context.h"

#include <cc_debug.h>

#include <errno.h>
#include <pthread.h>
#include <string.h>
#include <sysexits.h>

void
core_run(void *arg_worker)
{
    pthread_t worker, server, debug;
    int ret;

    if (!admin_init || !server_init || !worker_init) {
        log_crit("cannot run: admin/server/worker have to be initialized");
        return;
    }

    ret = pthread_create(&worker, NULL, core_worker_evloop, arg_worker);
    if (ret != 0) {
        log_crit("pthread create failed for worker thread: %s", strerror(ret));
        goto error;
    } else {
        log_info("worker thread of ID %d has been created", worker);
    }


    ret = pthread_create(&server, NULL, core_server_evloop, NULL);
    if (ret != 0) {
        log_crit("pthread create failed for server thread: %s", strerror(ret));
        goto error;
    } else {
        log_info("server thread of ID %d has been created", server);
    }

    if (debug_init) {
        ret = pthread_create(&debug, NULL, core_debug_evloop, NULL);
        if (ret != 0) {
            log_crit("pthread create failed for debug thread: %s", strerror(ret));
            goto error;
        } else {
            log_info("debug thread of ID %d has been created", debug);
        }
    }

    core_admin_evloop();

error:
    exit(EX_OSERR);
}
