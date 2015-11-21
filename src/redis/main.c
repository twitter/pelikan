#include <redis/setting.h>
#include <redis/stats.h>

#include <time/time.h>
#include <util/log_core.h>
#include <util/util.h>

#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_util.h>

#include <sysexits.h>

static struct setting setting = {
        SETTING(OPTION_INIT)
};

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
    option_printall_default((struct option *)&setting, nopt);
}

static void
setup(void)
{
    rstatus_i status;
    struct log_core *lc = NULL;

    /* Setup log */
    log_setup(&glob_stats.log_metrics);
    status = debug_setup((int)setting.debug_log_level.val.vuint,
                         setting.debug_log_file.val.vstr,
                         setting.debug_log_nbuf.val.vuint);
    if (status < 0) {
        log_error("log setup failed");
        goto error;
    }

    lc = log_core_create(dlog->logger, (int)setting.debug_log_intvl.val.vuint);
    if (lc == NULL) {
        log_stderr("Could not set up log core!");
        goto error;
    }

    /* daemonize */
    if (setting.daemonize.val.vbool) {
        daemonize();
    }

    /* create pid file, call it after daemonize to have the correct pid */
    if (setting.pid_filename.val.vstr != NULL) {
        create_pidfile(setting.pid_filename.val.vstr);
    }

    metric_setup();

    time_setup();
    procinfo_setup(&glob_stats.procinfo_metrics);

    return;

error:
    log_crit("setup failed");

    procinfo_teardown();
    time_teardown();
    metric_teardown();
    option_free((struct option *)&setting, nopt);

    log_core_destroy(&lc);
    debug_teardown();
    log_teardown();

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

    option_printall((struct option *)&setting, nopt);

    exit(EX_OK);
}
