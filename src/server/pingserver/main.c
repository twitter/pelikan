#include "setting.h"
#include "stats.h"

#include "../../time/time.h"
#include "../../util/util.h"

#include <cc_debug.h>

#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sysexits.h>

struct post_processor worker_processor = {
    pingserver_process_read,
    pingserver_process_write
};

static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
            "  pelikan_pingserver [option|config]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  pelikan_pingserver is, arguably, the most over-engineered " CRLF
            "  ping server. " CRLF
            CRLF
            "  The purpose is to demonstrate how to create an otherwise " CRLF
            "  minimal service with the libraries and modules provided by " CRLF
            "  Pelikan, which meets stringent requirements on latencies, " CRLF
            "  observability, configurability and other valuable traits " CRLF
            "  in a typical production environment." CRLF
            );
    log_stdout(
            "Command-line options:" CRLF
            "  -h, --help        show this message" CRLF
            "  -v, --version     show version number" CRLF
            "  -c, --config      list & describe all options in config" CRLF
            "  -s, --stats       list & describe all metrics in stats" CRLF
            );
    log_stdout(
            "Example:" CRLF
            "  pelikan_pingserver pingserver.conf" CRLF CRLF
            "Sample config files can be found under the config dir." CRLF
            );
}

static void
teardown(void)
{
    core_teardown();
    admin_process_teardown();
    compose_teardown();
    parse_teardown();
    procinfo_teardown();
    time_teardown();

    timing_wheel_teardown();
    tcp_teardown();
    sockio_teardown();
    event_teardown();
    dbuf_teardown();
    buf_teardown();

    debug_teardown();
    log_teardown();
}

static void
setup(void)
{
    char *fname = NULL;
    uint64_t intvl;

    if (atexit(teardown) != 0) {
        log_stderr("cannot register teardown procedure with atexit()");
        exit(EX_OSERR); /* only failure comes from NOMEM */
    }

    /* Setup logging first */
    log_setup(&stats.log);
    if (debug_setup(&setting.debug) < 0) {
        log_stderr("debug log setup failed");
        goto error;
    }

    /* setup top-level application options */
    if (option_bool(&setting.pingserver.daemonize)) {
        daemonize();
    }
    fname = option_str(&setting.pingserver.pid_filename);
    if (fname != NULL) {
        /* to get the correct pid, call create_pidfile after daemonize */
        create_pidfile(fname);
    }

    /* setup library modules */
    buf_setup(&setting.buf, &stats.buf);
    dbuf_setup(&setting.dbuf, &stats.dbuf);
    event_setup(&stats.event);
    sockio_setup(&setting.sockio);
    tcp_setup(&setting.tcp, &stats.tcp);
    timing_wheel_setup(&stats.timing_wheel);

    /* setup pelikan modules */
    time_setup();
    procinfo_setup(&stats.procinfo);
    parse_setup(&stats.parse_req, NULL);
    compose_setup(NULL, &stats.compose_rsp);
    admin_process_setup(&stats.admin_process);
    core_setup(&setting.admin, &setting.server, &setting.worker,
            &stats.server, &stats.worker);

    /* adding recurring events to maintenance/admin thread */
    intvl = option_uint(&setting.pingserver.dlog_intvl);
    if (core_admin_register(intvl, debug_log_flush, NULL) == NULL) {
        log_stderr("Could not register timed event to flush debug log");
        goto error;
    }

    return;

error:
    if (fname != NULL) {
        remove_pidfile(fname);
    }

    /* since we registered teardown with atexit, it'll be called upon exit */
    exit(EX_CONFIG);
}

int
main(int argc, char **argv)
{
    rstatus_i status = CC_OK;
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
        if (strcmp(argv[1], "-c") == 0 || strcmp(argv[1], "--config") == 0) {
            option_describe_all((struct option *)&setting, nopt);
            exit(EX_OK);
        }
        if (strcmp(argv[1], "-s") == 0 || strcmp(argv[1], "--stats") == 0) {
            metric_describe_all((struct metric *)&stats, nmetric);
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

    setup();
    option_print_all((struct option *)&setting, nopt);

    core_run(NULL, &worker_processor);

    exit(EX_OK);
}
