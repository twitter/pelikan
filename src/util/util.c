#include <util/util.h>

#include <cc_debug.h>
#include <cc_print.h>

#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sysexits.h>
#include <unistd.h>

void
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

void
show_version(void)
{
    log_stdout("Version: %s", VERSION_STRING);
}

rstatus_i
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

void
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

void
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
