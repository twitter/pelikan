#include "core.h"

#include "data/shared.h"

#include <cc_debug.h>
#include <cc_ring_array.h>
#include <channel/cc_pipe.h>

#include <errno.h>
#include <pthread.h>
#include <string.h>

struct buf_sock;
bool core_init = false;

/**
 * TODO: use function pointers to accommodate different channel types when we
 * extend to beyond just TCP
 */

void
core_setup(admin_options_st *opt_admin,
        server_options_st *opt_server, worker_options_st *opt_worker,
        server_metrics_st *smetrics, worker_metrics_st *wmetrics)
{
    core_shared_setup();

    core_server_setup(opt_server, smetrics);
    core_worker_setup(opt_worker, wmetrics);
    core_admin_setup(opt_admin);

    core_init = true;
}

void
core_teardown(void)
{
    core_admin_teardown();
    core_worker_teardown();
    core_server_teardown();

    core_shared_teardown();

    core_init = false;
}

void
core_run(void *arg_worker)
{
    pthread_t worker, server;
    int ret;

    if (!core_init) {
        log_crit("core cannot run because it hasn't been initialized");
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
    core_teardown();
}
