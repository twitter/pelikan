#include "admin/process.h"
#include "setting.h"
#include "stats.h"
#include "storage/cdb/cdb.h"

#include "time/time.h"
#include "util/util.h"

#include <cc_debug.h>
#include <cc_util.h>
#include <cc_bstring.h>

#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <sys/socket.h>
#include <sysexits.h>
#include <stdbool.h>
#include <stdlib.h>
#include <unistd.h>

struct data_processor worker_processor = {
    cdb_process_read,
    cdb_process_write,
    cdb_process_error,
};

struct CDBHandle *cdb_handle = NULL;

static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
            "  pelikan_cdb [option|config]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  pelikan_cdb is one of the unified cache backends. " CRLF
            "  It uses a slab-based storage to cache key/val pairs. " CRLF
            "  It speaks the memcached ASCII protocol and supports almost " CRLF
            "  all ASCII memcached commands." CRLF
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
            "  pelikan_cdb cdb.conf" CRLF CRLF
            "Sample config files can be found under the config dir." CRLF
            );
}

static void
teardown(void)
{
    core_worker_teardown();
    core_server_teardown();
    core_admin_teardown();
    admin_process_teardown();
    process_teardown();
    //slab_teardown();
    cdb_teardown();
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

static bool
setup_cdb_handle(void)
{
//    char *cdb_file_path = option_str(&setting.cdb.cdb_file_path);
//    if (cdb_file_path == NULL) {
//        log_stderr("cdb_file_path was not set!");
//        return false;
//    }
//    struct bstring fpath;
//    size_t fpath_len = 0;
//    bstring_init(&fpath);
//    fpath_len = cc_strlen(cdb_file_path);
//
//    fpath.data = cc_strndup(cdb_file_path, fpath_len);

    cdb_setup();

    char *cdb_file_path = strdup("/Users/jsimms/git/tub/cdb.rs/dict.cdb");
    cdb_handle = cdb_handle_create(cdb_file_path);

    return (cdb_handle == NULL);
}

static void
setup(void)
{
    char *fname = NULL;
    uint64_t intvl;

    if (!setup_cdb_handle()) {
        log_stderr("failed to set up cdb");
        goto error;
    }

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
    if (option_bool(&setting.cdb.daemonize)) {
        daemonize();
    }
    fname = option_str(&setting.cdb.pid_filename);
    if (fname != NULL) {
        /* to get the correct pid, call create_pidfile after daemonize */
        create_pidfile(fname);
    }


    /* setup library modules */
    buf_setup(&setting.buf, &stats.buf);
    dbuf_setup(&setting.dbuf, &stats.dbuf);
    event_setup(&stats.event);
    sockio_setup(&setting.sockio, &stats.sockio);
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

    //slab_setup(&setting.slab, &stats.slab);
    process_setup(&setting.process, &stats.process);
    admin_process_setup();
    core_admin_setup(&setting.admin);
    core_server_setup(&setting.server, &stats.server);
    core_worker_setup(&setting.worker, &stats.worker);

    /* adding recurring events to maintenance/admin thread */
    intvl = option_uint(&setting.cdb.dlog_intvl);
    if (core_admin_register(intvl, debug_log_flush, NULL) == NULL) {
        log_stderr("Could not register timed event to flush debug log");
        goto error;
    }

    intvl = option_uint(&setting.cdb.klog_intvl);
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

    core_run(&worker_processor);

    exit(EX_OK);
}
