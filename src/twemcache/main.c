#include <twemcache/setting.h>
#include <twemcache/stats.h>

#include <time/time.h>
#include <util/util.h>

#include <cc_debug.h>

#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sys/socket.h>
#include <sysexits.h>

static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
            "  pelikan_twemcache [option|config]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  pelikan_twemcache is one of the unified cache backends. " CRLF
            "  It uses a slab based key/val storage scheme to cache key/val" CRLF
            "  pairs. It speaks the memcached protocol and supports all " CRLF
            "  ASCII memcached commands." CRLF
            );
    log_stdout(
            "Options:" CRLF
            "  -h, --help        show this message" CRLF
            "  -v, --version     show version number" CRLF
            );
    log_stdout(
            "Example:" CRLF
            "  ./pelikan_twemcache ../template/twemcache.conf" CRLF
            );
    log_stdout("Setting & Default Values:");

    if (option_load_default((struct option *)&setting, nopt) != CC_OK) {
        log_stderr("failed to load default option values");
        exit(EX_CONFIG);
    }
    option_describe_all((struct option *)&setting, nopt);
}

static void
teardown(void)
{
    core_teardown();
    admin_process_teardown();
    process_teardown();
    slab_teardown();
    klog_teardown();
    compose_teardown();
    parse_teardown();
    response_teardown();
    request_teardown();
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
    if (debug_setup(&setting.debug) != CC_OK) {
        log_stderr("debug log setup failed");
        exit(EX_CONFIG);
    }

    /* setup top-level application options */
    if (option_bool(&setting.twemcache.daemonize)) {
        daemonize();
    }
    fname = option_str(&setting.twemcache.pid_filename);
    if (fname != NULL) {
        /* to get the correct pid, call create_pidfile after daemonize */
        create_pidfile(fname);
    }

    /* setup library modules */
    buf_setup(&setting.buf, &stats.buf);
    dbuf_setup(&setting.dbuf);
    event_setup(&stats.event);
    sockio_setup(&setting.sockio);
    tcp_setup(&setting.tcp, &stats.tcp);
    timing_wheel_setup(&stats.timing_wheel);

    /* setup pelikan modules */
    time_setup();
    procinfo_setup(&stats.procinfo);
    request_setup(&setting.request, &stats.request);
    response_setup(&setting.response, &stats.response);
    parse_setup(&stats.parse_req, NULL);
    compose_setup(NULL, &stats.compose_rsp);
    klog_setup(&setting.klog, &stats.klog);
    slab_setup(&setting.slab, &stats.slab);
    process_setup(&setting.process, &stats.process);
    admin_process_setup(&stats.admin_process);
    core_setup(&setting.admin, &setting.server, &setting.worker,
            &stats.server, &stats.worker);

    /* adding recurring events to maintenance/admin thread */
    intvl = option_uint(&setting.twemcache.dlog_intvl);
    if (core_admin_register(intvl, debug_log_flush, NULL) == NULL) {
        log_stderr("Could not register timed event to flush debug log");
        goto error;
    }

    intvl = option_uint(&setting.twemcache.klog_intvl);
    if (core_admin_register(intvl, klog_flush, NULL) == NULL) {
        log_error("Could not register timed event to flush command log");
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

    core_run();

    teardown();

    exit(EX_OK);
}
