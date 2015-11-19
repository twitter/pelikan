#include <core/core.h>

#include <core/shared.h>

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
#include <unistd.h>

struct buf_sock;
bool core_init = false;

/**
 * TODO: use function pointers to accommodate different channel types when we
 * extend to beyond just TCP
 */

rstatus_i
core_setup(struct addrinfo *data_ai, struct addrinfo *admin_ai,
           uint32_t max_conns, int maint_intvl,
           server_metrics_st *smetrics, worker_metrics_st *wmetrics)
{
    rstatus_i ret;

    pipe_c = pipe_conn_create();

    if (pipe_c == NULL) {
        log_error("Could not create connection for pipe, abort");
        return CC_ERROR;
    }

    if (!pipe_open(NULL, pipe_c)) {
        log_error("Could not open pipe connection: %s", strerror(pipe_c->err));
        return CC_ERROR;
    }

    pipe_set_nonblocking(pipe_c);

    conn_arr = ring_array_create(sizeof(struct buf_sock *), max_conns);
    if (conn_arr == NULL) {
        log_error("core setup failed: could not allocate conn array");
        return CC_ERROR;
    }

    ret = core_server_setup(data_ai, smetrics);
    if (ret != CC_OK) {
        return ret;
    }

    ret = core_worker_setup(wmetrics);
    if (ret != CC_OK) {
        return ret;
    }

    ret = admin_setup(admin_ai, maint_intvl);
    if (ret != CC_OK) {
        return ret;
    }

    core_init = true;
    return CC_OK;
}

void
core_teardown(void)
{
    core_server_teardown();
    core_worker_teardown();

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

    ret = pthread_create(&admin, NULL, admin_evloop, NULL);
    if (ret != 0) {
        log_crit("pthread create failed for admin thread: %s", strerror(ret));
        goto error;
    }

    core_server_evloop();

error:
    core_teardown();
}
