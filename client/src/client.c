#include <client_core.h>
#include <client_setting.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_log.h>
#include <cc_option.h>
#include <channel/cc_tcp.h>

#include <stdio.h>
#include <stdlib.h>
#include <sysexits.h>

static struct setting setting = {
    SETTING(OPTION_INIT)
};

#define PRINT_DEFAULT(_name, _type, _default, _description) \
    log_stdout("  %-31s ( default: %s )", #_name,  _default);

static const unsigned int nopt = OPTION_CARDINALITY(struct setting);

static void
show_usage(void)
{
    log_stdout(
        "Usage:" CRLF
        "  pelikan_client [option|config]" CRLF
        );
    log_stdout(
        "Description:" CRLF
        "  pelikan_client is an integration test/testing client for the " CRLF
        "  pelikan backends." CRLF
        );
    log_stdout(
        "Options:" CRLF
        "  -h, --help        show this message" CRLF
        );
    log_stdout(
        "Example:" CRLF
        "./pelikan_client ../template/client.conf" CRLF
        );
    log_stdout("Setting & Default Values:");
    SETTING(PRINT_DEFAULT)
}

static rstatus_t
getaddr(struct addrinfo **ai, char *hostname, char *servname)
{
    int ret;
    struct addrinfo hints = { .ai_flags = AI_PASSIVE, .ai_family = AF_UNSPEC,
                              .ai_socktype = SOCK_STREAM };

    ret = getaddrinfo(hostname, servname, &hints, ai);

    if (ret != 0) {
        log_error("cannot resolve address: %s", gai_strerror(ret));
        return CC_ERROR;
    }

    return CC_OK;
}

static void
setup(void)
{
    struct addrinfo *ai;
    rstatus_t status;

    log_setup(NULL);
    status = debug_setup((int)setting.debug_log_level.val.vuint,
                         setting.debug_log_file.val.vstr,
                         setting.debug_log_nbuf.val.vuint);

    if (status < 0) {
        log_stderr("Log setup failed");
        goto error;
    }

    tcp_setup((int)setting.tcp_backlog.val.vuint, NULL);

    status = getaddr(&ai, setting.server_host.val.vstr,
                     setting.server_port.val.vstr);

    client_core_setup(ai);

    return;

error:
    tcp_teardown();
    debug_teardown();
    log_teardown();

    log_crit("setup failed");

    exit(EX_CONFIG);
}

int
main(int argc, char *argv[])
{
    rstatus_t status = CC_OK;

    FILE *fp = NULL;
    if (argc > 2) {
        show_usage();
        exit(EX_USAGE);
    }

    if (argc == 1) {
        log_stderr("launching server with default values.");
    } else {
        /* argc == 2 */
        if (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
            show_usage();
            exit(EX_OK);
        }

        fp = fopen(argv[1], "r");
        if (fp == NULL) {
            log_stderr("cannot open config: incorrect path or doesn't exist");
            exit(EX_DATAERR);
        }
    }

    if (option_load_default((struct option *)&setting, nopt) != CC_OK) {
        log_stderr("failed to load default option values");
        exit(EX_CONFIG);
    }

    if (fp != NULL) {
        log_stderr("load config from %s", argv[1]);
        status = option_load_file(fp, (struct option *)&setting, nopt);
        fclose(fp);
    }
    if (status != CC_OK) {
        log_stderr("failed to load config");
        exit(EX_DATAERR);
    }

    option_printall((struct option *)&setting, nopt);

    setup();

    client_core_run();

    return 0;
}
