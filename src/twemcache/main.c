#include <twemcache/data/process.h>
#include <twemcache/setting.h>
#include <twemcache/stats.h>

#include <core/core.h>
#include <protocol/memcache/klog.h>
#include <storage/slab/item.h>
#include <storage/slab/slab.h>
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

static struct setting setting = {
    SETTING(OPTION_INIT)
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
    option_printall_default((struct option *)&setting, nopt);
}

static void
setup(void)
{
    struct addrinfo *data_ai, *admin_ai;
    uint32_t max_conns;
    rstatus_i status;

    /* Setup log */
    log_setup(&glob_stats.log_metrics);
    status = debug_setup((int)setting.debug_log_level.val.vuint,
                         setting.debug_log_file.val.vstr,
                         setting.debug_log_nbuf.val.vuint,
                         setting.debug_log_intvl.val.vuint);
    if (status < 0) {
        log_error("log setup failed");
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
    timing_wheel_setup(&glob_stats.timing_wheel_metrics);
    procinfo_setup(&glob_stats.procinfo_metrics);
    request_setup(&glob_stats.request_metrics);
    response_setup(&glob_stats.response_metrics);
    parse_setup(&glob_stats.parse_req_metrics, NULL);
    compose_setup(NULL, &glob_stats.compose_rsp_metrics);
    klog_setup(setting.klog_file.val.vstr, (uint32_t)setting.klog_nbuf.val.vuint,
               (uint32_t)setting.klog_intvl.val.vuint, (uint32_t)setting.klog_sample.val.vuint,
               &glob_stats.klog_metrics);
    process_setup(setting.allow_flush.val.vbool,
                  &glob_stats.process_metrics);
    admin_process_setup(&glob_stats.admin_process_metrics);

    buf_setup((uint32_t)setting.buf_init_size.val.vuint, &glob_stats.buf_metrics);
    dbuf_setup((uint32_t)setting.dbuf_max_power.val.vuint);
    event_setup(&glob_stats.event_metrics);
    tcp_setup((int)setting.tcp_backlog.val.vuint, &glob_stats.tcp_metrics);

    if (slab_setup((uint32_t)setting.slab_size.val.vuint,
                   setting.slab_prealloc.val.vbool,
                   (int)setting.slab_evict_opt.val.vuint,
                   setting.slab_use_freeq.val.vbool,
                   (size_t)setting.slab_min_chunk_size.val.vuint,
                   (size_t)setting.slab_max_chunk_size.val.vuint,
                   (size_t)setting.slab_maxbytes.val.vuint,
                   setting.slab_profile.val.vstr,
                   setting.slab_profile_factor.val.vstr,
                   &glob_stats.slab_metrics)
        != CC_OK) {
        log_error("slab module setup failed");
        goto error;
    }
    if (item_setup(setting.item_use_cas.val.vbool,
                   (uint32_t)setting.item_hash_power.val.vuint,
                   &glob_stats.item_metrics)
        != CC_OK) {
        log_error("item setup failed");
        goto error;
    }

    buf_sock_pool_create((uint32_t)setting.buf_sock_poolsize.val.vuint);
    request_pool_create((uint32_t)setting.request_poolsize.val.vuint);
    response_pool_create((uint32_t)setting.response_poolsize.val.vuint);

    /* set up core */
    status = getaddr(&data_ai, setting.server_host.val.vstr,
                     setting.server_port.val.vstr);

    if (status != CC_OK) {
        log_error("server address invalid");
        goto error;
    }

    status = getaddr(&admin_ai, setting.admin_host.val.vstr,
                     setting.admin_port.val.vstr);
    if (status != CC_OK) {
        log_error("admin address invalid");
        goto error;
    }

    /* Set up core with connection ring array being either the tcp poolsize or
       the ring array default capacity if poolsize is unlimited */

    max_conns = setting.tcp_poolsize.val.vuint == 0 ?
        setting.ring_array_cap.val.vuint : setting.tcp_poolsize.val.vuint;
    status = core_setup(data_ai, admin_ai, max_conns,
        (int)setting.admin_intvl.val.vuint, setting.admin_tw_tick.val.vuint,
        setting.admin_tw_cap.val.vuint, setting.admin_tw_ntick.val.vuint,
        &glob_stats.server_metrics, &glob_stats.worker_metrics);
    freeaddrinfo(data_ai);
    freeaddrinfo(admin_ai);

    if (status != CC_OK) {
        log_crit("could not start core event loop");
        goto error;
    }

    status = admin_add_timed_ev(dlog_tev);
    if (status != CC_OK) {
        log_stderr("Could not add debug log timed event to admin thread");
        goto error;
    }

    status = admin_add_timed_ev(klog_tev);
    if (status != CC_OK) {
        log_error("Could not add klog timed event to admin thread");
        goto error;
    }

    return;

error:
    log_crit("setup failed");

    if (setting.pid_filename.val.vstr != NULL) {
        remove_pidfile(setting.pid_filename.val.vstr);
    }

    core_teardown();

    request_pool_destroy();
    buf_sock_pool_destroy();
    tcp_conn_pool_destroy();

    item_teardown();
    slab_teardown();
    dbuf_teardown();
    buf_teardown();
    process_teardown();
    klog_teardown();
    compose_teardown();
    parse_teardown();
    response_teardown();
    request_teardown();
    tcp_teardown();
    event_teardown();
    procinfo_teardown();
    time_teardown();
    metric_teardown();
    option_free((struct option *)&setting, nopt);

    debug_teardown();
    log_teardown();

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

    option_printall((struct option *)&setting, nopt);

    core_run();

    exit(EX_OK);
}
