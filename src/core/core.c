#include <core/core.h>

#include <core/data/shared.h>

#include <cc_debug.h>
#include <cc_ring_array.h>
#include <channel/cc_pipe.h>

#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <pthread.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sysexits.h>
#include <unistd.h>

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
    pipe_c = pipe_conn_create();
    if (pipe_c == NULL) {
        log_error("Could not create connection for pipe, abort");
        goto error;
    }

    if (!pipe_open(NULL, pipe_c)) {
        log_error("Could not open pipe connection: %s", strerror(pipe_c->err));
        goto error;
    }

    pipe_set_nonblocking(pipe_c);

    conn_arr = ring_array_create(sizeof(struct buf_sock *), RING_ARRAY_DEFAULT_CAP);
    if (conn_arr == NULL) {
        log_error("core setup failed: could not allocate conn array");
        goto error;
    }

    core_server_setup(opt_server, smetrics);
    core_worker_setup(opt_worker, wmetrics);
    core_admin_setup(opt_admin);

    core_init = true;

    return;

error:
    exit(EX_CONFIG);
}

void
core_teardown(void)
{
    core_admin_teardown();
    core_worker_teardown();
    core_server_teardown();

    ring_array_destroy(conn_arr);
    pipe_conn_destroy(&pipe_c);

    core_init = false;
}

void
core_run(void)
{
    pthread_t worker, admin;
    int ret;

    if (!core_init) {
        log_crit("core cannot run because it hasn't been initialized");
        return;
    }

    ret = pthread_create(&worker, NULL, core_worker_evloop, NULL);
    if (ret != 0) {
        log_crit("pthread create failed for worker thread: %s", strerror(ret));
        goto error;
    }

    __atomic_store_n(&admin_running, true, __ATOMIC_RELAXED);
    ret = pthread_create(&admin, NULL, core_admin_evloop, NULL);
    if (ret != 0) {
        log_crit("pthread create failed for admin thread: %s", strerror(ret));
        goto error;
    }

    core_server_evloop();

error:
    core_teardown();
}
