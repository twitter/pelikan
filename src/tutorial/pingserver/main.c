#include <util/util.h>

#include <cc_log.h>

#include <stdlib.h>
#include <string.h>
#include <sysexits.h>

static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
    "  pelikan_pingserver <option>" CRLF
    );
    log_stdout(
            "Options:" CRLF
    "  -h, --help        show this message" CRLF
    "  -v, --version     show version number" CRLF
    );

}

int
main(int argc, char **argv)
{
    if (argc != 2) {
        show_usage();
        exit(EX_USAGE);
    }

    if (strcmp(argv[1], "-h") == 0 || strcmp(argv[1], "--help") == 0) {
        show_usage();

        exit(EX_OK);
    }
    if (strcmp(argv[1], "-v") == 0 || strcmp(argv[1], "--version") == 0) {
        show_version();

        exit(EX_OK);
    }
}

