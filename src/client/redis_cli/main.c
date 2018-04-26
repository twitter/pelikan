#include "setting.h"

#include <errno.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <sysexits.h>


static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
            "  pelikan_slimredis [option|config]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  pelikan_slimredis is one of the unified cache backends. " CRLF
            "  It uses managed storage backends to cache key/val pairs. " CRLF
            "  It speaks the memcached ASCII protocol and supports almost " CRLF
            "  all ASCII memcached commands." CRLF
            );
    log_stdout(
            "Command-line options:" CRLF
            "  -h, --help        show this message" CRLF
            "  -v, --version     show version number" CRLF
            "  -c, --config      list & describe all options in config" CRLF
            );
    log_stdout(
            "Example:" CRLF
            "  pelikan_slimredis slimredis.conf" CRLF CRLF
            "Sample config files can be found under the config dir." CRLF
            );
}

static void
teardown(void)
{
    compose_teardown();
    parse_teardown();
    response_teardown();
    request_teardown();

    tcp_teardown();
    sockio_teardown();
    dbuf_teardown();
    buf_teardown();

    debug_teardown();
    log_teardown();
}

static void
setup(void)
{
    if (atexit(teardown) != 0) {
        log_stderr("cannot register teardown procedure with atexit()");
        exit(EX_OSERR); /* only failure comes from NOMEM */
    }

    /* setup library modules */
    buf_setup(&setting.buf, NULL);
    dbuf_setup(&setting.dbuf, NULL);
    sockio_setup(&setting.sockio, NULL);
    tcp_setup(&setting.tcp, NULL);

    /* setup pelikan modules */
    request_setup(&setting.request, NULL);
    response_setup(&setting.response, NULL);
    parse_setup(NULL, NULL);
    compose_setup(NULL, NULL);

    return;
}

int
main(int argc, char **argv)
{
    rstatus_i status = CC_OK;;
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
        if (strcmp(argv[1], "-v") == 0 || strcmp(argv[1], "--version") == 0) {
            show_version();
            exit(EX_OK);
        }
    }

    if (option_load_default((struct option *)&setting, nopt) != CC_OK) {
        log_stderr("failed to load default option values");
        exit(EX_CONFIG);
    }

    /* TODO(yao): modify option module in ccommon to allow ignore unmatched
     * option, this will allow us to reuse server config files with cli
     */
    if (fp != NULL) {
        log_stderr("load config from %s", argv[1]);
        status = option_load_file(fp, (struct option *)&setting, nopt);
        fclose(fp);
    }
    if (status != CC_OK) {
        log_stderr("failed to load config");
        exit(EX_DATAERR);
    }

    setup();

    exit(EX_OK);
}
