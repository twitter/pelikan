#include "shared.h"

#include <cc_ring_array.h>
#include <channel/cc_pipe.h>

#include <errno.h>
#include <string.h>
#include <sysexits.h>


/* needs to be initialized to avoid linker issues due to being optimized out */
struct pipe_conn *pipe_c = NULL;

struct ring_array *conn_arr = NULL;

void
core_shared_setup(void)
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

error:
    exit(EX_CONFIG);
}


void
core_shared_teardown(void)
{
    ring_array_destroy(conn_arr);
    pipe_conn_destroy(&pipe_c);
}

