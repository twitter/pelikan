#include <util/bb_core.h>

#include <util/bb_core_shared.h>

#include <cc_ring_array.h>

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

rstatus_t
core_setup(struct addrinfo *ai, uint32_t max_conns, server_metrics_st *smetrics,
        worker_metrics_st *wmetrics)
{
    rstatus_t ret;
    int status;

    /* TODO(kyang): create pipe implementation of channel interface in ccommon,
       and replace conn_fds with that */
    status = pipe(conn_fds);
    if (status) {
        log_error("pipe failed: %s", strerror(errno));
        return CC_ERROR;
    }

    /* set non-blocking flag for conn_fds */
    fcntl(conn_fds[0], F_SETFL, O_NONBLOCK);
    fcntl(conn_fds[1], F_SETFL, O_NONBLOCK);

    conn_arr = ring_array_create(sizeof(struct buf_sock *), max_conns);
    if (conn_arr == NULL) {
        log_error("core setup failed: could not allocate conn array");
        return CC_ERROR;
    }

    ret = core_server_setup(ai, smetrics);
    if (ret != CC_OK) {
        return ret;
    }

    ret = core_worker_setup(wmetrics);
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
    core_init = false;
}

void
core_run(void)
{
    pthread_t worker;
    int ret;

    if (!core_init) {
        log_crit("core cannot run because it hasn't been initialized");
        return;
    }

    ret = pthread_create(&worker, NULL, core_worker_evloop, NULL);

    if (ret != 0) {
        log_crit("pthread create failed for worker thread: %s", strerror(ret));
    } else {
        core_server_evloop();
    }

    core_teardown();
}
