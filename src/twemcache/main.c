#include <twemcache/setting.h>
#include <twemcache/stats.h>

#include <time/time.h>
#include <util/util.h>

#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_option.h>

#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sys/socket.h>
#include <sysexits.h>

static struct setting setting = (struct setting) {
    { TWEMCACHE_OPTION(OPTION_INIT) },
    { ADMIN_OPTION(OPTION_INIT)     },
    { SERVER_OPTION(OPTION_INIT)    },
    { WORKER_OPTION(OPTION_INIT)    },
    { PROCESS_OPTION(OPTION_INIT)   },
    { KLOG_OPTION(OPTION_INIT)      },
    { REQUEST_OPTION(OPTION_INIT)   },
    { RESPONSE_OPTION(OPTION_INIT)  },
    { SLAB_OPTION(OPTION_INIT)      },
    { ARRAY_OPTION(OPTION_INIT)     },
    { BUF_OPTION(OPTION_INIT)       },
    { DBUF_OPTION(OPTION_INIT)      },
    { DEBUG_OPTION(OPTION_INIT)     },
    { SOCKIO_OPTION(OPTION_INIT)    },
    { TCP_OPTION(OPTION_INIT)       },
};

static const unsigned int nopt = OPTION_CARDINALITY(struct setting);

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

    /* Setup logging first */
    log_setup(&stats.log);
    if (debug_setup(&setting.debug) < 0) {
        log_error("log setup failed");
        goto error;
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
    if (klog_setup(&setting.klog, &stats.klog) != CC_OK) {
        log_error("klog setup failed");
        goto error;
    }
    if (slab_setup(&setting.slab, &stats.slab) != CC_OK) {
        log_error("slab setup failed");
        goto error;
    }
    process_setup(&setting.process, &stats.process);
    admin_process_setup(&stats.admin_process);
    core_setup(&setting.admin, &setting.server, &setting.worker,
            &stats.server, &stats.worker);

    /* adding recurring events to maintenance/admin thread */
    if (core_admin_add_tev(dlog_tev) != CC_OK) {
        log_stderr("Could not add debug log timed event to admin thread");
        goto error;
    }
    if (core_admin_add_tev(klog_tev) != CC_OK) {
        log_error("Could not add klog timed event to admin thread");
        goto error;
    }

    return;

error:
    log_crit("setup failed");

    /* tear down everything in the reverse order as setup, then exit */
    teardown();
    if (fname != NULL) {
        remove_pidfile(fname);
    }

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
