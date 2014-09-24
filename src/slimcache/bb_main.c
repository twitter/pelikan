#include <slimcache/bb_core.h>

#include <cc_mbuf.h>
#include <cc_nio.h>
#include <cc_stream.h>

#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>

static int
getaddr(struct addrinfo **ai)
{
    struct addrinfo hints = { .ai_flags = AI_PASSIVE, .ai_family = AF_UNSPEC,
        .ai_socktype = SOCK_STREAM };
    int ret;

    ret = getaddrinfo("127.0.0.1", "22222", &hints, ai);
    if (ret < 0) {
        log_error("cannot resolve address");
    }

    return ret;
}

static void
run(void)
{
    rstatus_t status;

    for (;;) {
        status = core_evwait();
        if (status != CC_OK) {
            log_debug(LOG_CRIT, "core event loop exits due to failure");
            break;
        }
    }

    core_teardown();
}

static rstatus_t
setup(void)
{
    struct addrinfo *ai;
    int ret;
    rstatus_t status;

    mbuf_setup(8 * KiB);
    array_setup(64);
    log_setup(LOG_VERB, NULL);

    mbuf_pool_create(0);
    conn_pool_create(0);
    stream_pool_create(0);

    ret = getaddr(&ai);
    if (ret < 0) {
        log_error("address invalid");

        return CC_ERROR;
    }

    status = core_setup(ai);
    if (status != CC_OK) {
        log_debug(LOG_CRIT, "cannot start core event loop");

        return CC_ERROR;
    }

    return CC_OK;
}

int
main(int argc, char **argv)
{
    rstatus_t status;

    status = setup();
    if (status != CC_OK) {
        log_debug(LOG_CRIT, "setup failed");

        return -1;
    }

    run();

    return 0;
}
