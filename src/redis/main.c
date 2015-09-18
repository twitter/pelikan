#include <redis/setting.h>

#include <util/util.h>

#include <cc_debug.h>
#include <cc_option.h>
#include <cc_util.h>

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
            "  pelikan_redis [option]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  pelikan_redis is one of the unified cache backends. " CRLF
            "  It speaks the redis protocol and supports only a " CRLF
            "  subset of original redis commands." CRLF
            );
    log_stdout(
            "Options:" CRLF
            "  -h, --help        show this message" CRLF
            "  -v, --version     show version number" CRLF
            );
    log_stdout(
            "Example:" CRLF
            "  ./pelikan_redis" CRLF
            );
    log_stdout("Setting & Default Values:");
    SETTING(PRINT_DEFAULT)
}

int
main(int argc, char **argv)
{
    rstatus_t status = CC_OK;;

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

        log_stderr("unknown option");
        exit(EX_USAGE);
    }

    if (status != CC_OK) {
        log_stderr("failed to load config");

        exit(EX_DATAERR);
    }

    option_printall((struct option *)&setting, nopt);

    exit(EX_OK);
}
