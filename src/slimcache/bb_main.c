#include <cuckoo/bb_cuckoo.h>
#include <memcache/bb_request.h>
#include <slimcache/bb_core.h>

#include <cc_array.h>
#include <cc_log.h>
#include <cc_mbuf.h>
#include <cc_nio.h>
#include <cc_stream.h>

#include <netdb.h>
#include <stdio.h>
#include <string.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <sysexits.h>

/*          name        type            default     description */
#define SERVER_OPTION(ACTION)                                                   \
    ACTION( host,   OPTION_TYPE_STR,    NULL,       "interfaces listening on"  )\
    ACTION( port,   OPTION_TYPE_STR,    "22222",    "port listening on"        )

/* we compose our setting by including options needed by modules we use */
#define SETTING(ACTION)             \
    ARRAY_OPTION(ACTION)            \
    CUCKOO_OPTION(ACTION)           \
    ITEM_OPTION(ACTION)             \
    MBUF_OPTION(ACTION)             \
    NIO_OPTION(ACTION)              \
    SERVER_OPTION(ACTION)           \
    STREAM_OPTION(ACTION)

static struct setting {
    SETTING(OPTION_DECLARE)
} setting = {
    SETTING(OPTION_INIT)
};

const unsigned int nopt = OPTION_CARDINALITY(struct setting);

static void
show_usage(void)
{
    log_stderr(
            "Usage:" CRLF
            "  broadbill_slimcache [option|config]" CRLF
            CRLF
            "Description:" CRLF
            "broadbill_slimcache is a part of the unified cache backend " CRLF
            "that uses cuckoo hashing to efficiently store small key/val " CRLF
            "pairs. It speaks the memcached protocol and supports all " CRLF
            "ASCII memcached commands except for prepend/append. " CRLF
            "The storage is preallocated and maximum key/val size allowed " CRLF
            "has to be specified when starting the service, and cannot be " CRLF
            "updated after launch." CRLF
            CRLF
            "Options:" CRLF
            "  -h, --help        show this message" CRLF
            "  -v, --version     show version number" CRLF
            CRLF
            "Defaults:" CRLF
            CRLF
            "Example:" CRLF
            "  ./broadbill_slimcache ../template/slimcache.config" CRLF
            );
}

static void
show_version(void)
{
    log_stderr("Version: %s", BB_VERSION_STRING);
}

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
            log_crit("core event loop exits due to failure");
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
    request_pool_create(0);
    cuckoo_setup(80, 1024);

    ret = getaddr(&ai);
    if (ret < 0) {
        log_error("address invalid");

        return CC_ERROR;
    }

    status = core_setup(ai);
    if (status != CC_OK) {
        log_crit("cannot start core event loop");

        return CC_ERROR;
    }

    return CC_OK;
}

int
main(int argc, char **argv)
{
    rstatus_t status;
    FILE *fp = NULL;

    if (argc > 2) {
        show_usage();
        exit(EX_USAGE);
    }

    if (argc == 1) {
        log_stderr("launching server with default values.");
    }

    if (argc == 2) {
        if (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
            show_usage();

            exit(EX_OK);
        }
        if (strcmp(argv[1], "-v") == 0 || strcmp(argv[1], "--version") == 0) {
            show_version();

            exit(EX_OK);
        }
        fp = fopen(argv[1], "r");
        if (fp == NULL) {
            log_stderr("cannot open config: incorrect path or doesn't exist");

            exit(EX_DATAERR);
        }
    }

    status = option_load_default((struct option *)&setting, nopt);
    if (status != CC_OK) {
        log_stderr("fail to load default option values");

        exit(EX_CONFIG);
    }
    if (fp != NULL) {
        log_stderr("load config from %s", argv[1]);
        status = option_load_file(fp, (struct option *)&setting, nopt);
        fclose(fp);
    }
    if (status != CC_OK) {
        log_stderr("fail to load config");

        exit(EX_DATAERR);
    }
    option_printall((struct option *)&setting, nopt);

    status = setup();
    if (status != CC_OK) {
        log_crit("setup failed");

        exit(EX_CONFIG);
    }

    run();

    exit(EX_OK);
}
