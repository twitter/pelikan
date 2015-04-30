#include <twemcache/bb_core.h>
#include <twemcache/bb_setting.h>
#include <twemcache/bb_stats.h>

#include <storage/slab/bb_item.h>
#include <storage/slab/bb_slab.h>
#include <time/bb_time.h>

#include <cc_log.h>
#include <cc_option.h>
#include <cc_print.h>
#include <cc_signal.h>

#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sysexits.h>
#include <unistd.h>

static struct setting setting = {
    SETTING(OPTION_INIT)
};

#define PRINT_DEFAULT(_name, _type, _default, _description) \
    log_stdout("  %-31s ( default: %s )", #_name,  _default);

static const unsigned int nopt = OPTION_CARDINALITY(struct setting);

struct stats Stats = {
    STATS(METRIC_INIT)
};

const unsigned int Nmetric = METRIC_CARDINALITY(struct stats);

static void
show_usage(void)
{
    log_stdout(
            "Usage:" CRLF
            "  broadbill_twemcache [option|config]" CRLF
            );
    log_stdout(
            "Description:" CRLF
            "  broadbill_twemcache is one of the unified cache backends. " CRLF
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
            "  ./broadbill_slimcache ../template/slimcache.config" CRLF
            );
    log_stdout("Setting & Default Values:");
    SETTING(PRINT_DEFAULT)
}

/* TODO(kyang): make this common w/ the one in slimcache/bb_main */
static void
show_version(void)
{
    log_stdout("Version: %s", BB_VERSION_STRING);
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
daemonize(void)
{
    pid_t pid, sid;
    int fd;
    int ret;

    /* 1st fork detaches child from terminal */
    pid = fork();
    switch (pid) {
    case -1:
        log_error("fork() failed: %s", strerror(errno));
        goto error;

    case 0:
        break;

    default:
        /* parent terminates */
        _exit(0);
    }

    /* 1st child continues and becomes the session and process group leader */
    sid = setsid();
    if (sid < 0) {
        goto error;
    }

    /* 2nd fork turns child into a non-session leader: cannot acquire terminal */
    pid = fork();
    switch (pid) {
    case -1:
        log_error("fork() failed: %s", strerror(errno));
	goto error;

    case 0:
        break;

    default:
        /* 1st child terminates */
        _exit(0);
    }

    /* TODO: add option to change directory to root */

    /* clear file mode creation mask */
    umask(0);

    /* redirect stdin, stdout and stderr to "/dev/null" */

    fd = open("/dev/null", O_RDWR);
    if (fd < 0) {
        log_error("open(\"/dev/null\") failed: %s", strerror(errno));
	exit(EX_CANTCREAT);
    }

    ret = dup2(fd, STDIN_FILENO);
    if (ret < 0) {
        log_error("dup2(%d, STDIN) failed: %s", fd, strerror(errno));
	goto fderror;
    }

    ret = dup2(fd, STDOUT_FILENO);
    if (ret < 0) {
        log_error("dup2(%d, STDOUT) failed: %s", fd, strerror(errno));
 	goto error;
    }

    ret = dup2(fd, STDERR_FILENO);
    if (ret < 0) {
        log_error("dup2(%d, STDERR) failed: %s", fd, strerror(errno));
	goto error;
    }

    if (fd > STDERR_FILENO) {
        ret = close(fd);
        if (ret < 0) {
            log_error("close(%d) failed: %s", fd, strerror(errno));
	    exit(EX_SOFTWARE);
        }
    }

    log_info("process daemonized");

    return;

error:
    exit(EX_OSERR);

fderror:
    close(fd);
    exit(EX_CANTCREAT);
}

static void
create_pidfile(const char *filename)
{
    int ret;
    char pid_str[CC_UINTMAX_MAXLEN];
    int fd, pid_len;
    ssize_t n;

    ASSERT(filename != NULL);

    fd = open(filename, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        log_error("open pid file '%s' failed: %s", filename, strerror(errno));

	exit(EX_CANTCREAT);
    }

    pid_len = cc_snprintf(pid_str, CC_UINTMAX_MAXLEN, "%d", getpid());

    n = write(fd, pid_str, pid_len);
    if (n < 0) {
        log_error("write to pid file '%s' failed: %s", filename,
                  strerror(errno));

	exit(EX_IOERR);
    }

    ret = close(fd);
    if (ret< 0) {
        log_warn("close pid file '%s' failed: %s", filename, strerror(errno));
    }

    log_info("wrote pid %d to file %s", getpid(), filename);
}

static void
remove_pidfile(const char *filename)
{
    int ret;

    ASSERT(filename != NULL);

    ret = unlink(filename);
    if (ret < 0) {
        log_warn("unlink/remove of pid file '%s' failed, ignored: %s",
                  filename, strerror(errno));
    }
}

static void
run(void)
{
    rstatus_t status;

    for (;;) {
        status = core_evwait();
        if (status != CC_OK) {
            log_crit("core event loop exited due to failure");
            break;
        }
    }

    core_teardown();
}

static void
setup(void)
{
    struct addrinfo *ai;
    /* int ret; */
    rstatus_t status;

    /* Setup log */
    if (log_setup((int)setting.log_level.val.vuint,
                 setting.log_name.val.vstr) < 0) {
        log_error("log setup failed");
        goto error;
    }

    time_setup();

    buf_setup((uint32_t)setting.buf_size.val.vuint);
    dbuf_setup((uint32_t)setting.dbuf_max_size.val.vuint,
               (uint32_t)setting.dbuf_shrink_factor.val.vuint);

    if (slab_setup((uint32_t)setting.slab_size.val.vuint,
                   setting.slab_use_cas.val.vbool,
                   setting.slab_prealloc.val.vbool,
                   (int)setting.slab_evict_opt.val.vuint,
                   setting.slab_use_freeq.val.vbool,
                   (size_t)setting.slab_chunk_size.val.vuint,
                   (size_t)setting.slab_maxbytes.val.vuint,
                   setting.slab_profile.val.vstr,
                   (uint8_t)setting.slab_profile_last_id.val.vuint)
        != CC_OK) {
        log_error("slab module setup failed");
        goto error;
    }

    if (item_setup((uint32_t)setting.slab_hash_power.val.vuint) != CC_OK) {
        log_error("item module setup failed");
        goto error;
    }

    buf_sock_pool_create((uint32_t)setting.buf_sock_poolsize.val.vuint);
    request_pool_create((uint32_t)setting.request_poolsize.val.vuint);

    /* set up core */
    status = getaddr(&ai, setting.server_host.val.vstr,
                     setting.server_port.val.vstr);

    if(status != CC_OK) {
        log_error("address invalid");
        goto error;
    }
    status = core_setup(ai);
    freeaddrinfo(ai);

    if (status != CC_OK) {
        log_crit("could not start core event loop");
        goto error;
    }

    /* Not overriding signals for now, since we are still testing */

    /* override signals that we want to customize */
    /* ret = signal_segv_stacktrace(); */
    /* if (ret < 0) { */
    /*     goto error; */
    /* } */

    /* ret = signal_ttin_logrotate(); */
    /* if (ret < 0) { */
    /*     goto error; */
    /* } */

    /* ret = signal_pipe_ignore(); */
    /* if (ret < 0) { */
    /*     goto error; */
    /* } */

    /* daemonize */
    if (setting.daemonize.val.vbool) {
        daemonize();
    }

    /* create pid file, call it after daemonize to have the correct pid */
    if (!option_empty(&setting.pid_filename)) {
        create_pidfile(setting.pid_filename.val.vstr);
    }

    return;

error:
    if (!option_empty(&setting.pid_filename)) {
        remove_pidfile(setting.pid_filename.val.vstr);
    }

    core_teardown();

    request_pool_destroy();
    buf_sock_pool_destroy();
    conn_pool_destroy();

    item_teardown();
    slab_teardown();
    dbuf_teardown();
    buf_teardown();
    time_teardown();

    log_teardown();

    log_crit("setup failed");

    exit(EX_CONFIG);
}

int
main(int argc, char **argv)
{
    rstatus_t status = CC_OK;;
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

    option_printall((struct option *)&setting, nopt);

    setup();

    run();

    exit(EX_OK);
}
