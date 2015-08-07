#include <cc_signal.h>

#include <cc_bstring.h>
#include <cc_define.h>
#include <cc_debug.h>

#include <errno.h>
#include <signal.h>
#include <string.h>

#ifndef CC_HAVE_SIGNAME
const char* sys_signame[SIGNAL_MAX + 1] = {
    "UNDEFINED",
    "SIGHUP",
    "SIGINT",
    "SIGQUIT",
    "SIGILL",
    "SIGTRAP",
    "SIGABRT",
    "SIGEMT",
    "SIGFPE",
    "SIGKILL",
    "SIGBUS",
    "SIGSEGV",
    "SIGSYS",
    "SIGPIPE",
    "SIGALRM",
    "SIGTERM",
    "SIGURG",
    "SIGSTOP",
    "SIGTSTP",
    "SIGCONT",
    "SIGCHLD",
    "SIGTTIN",
    "SIGTTOU",
    "SIGIO",
    "SIGXCPU",
    "SIGXFSZ",
    "SIGVTALRM",
    "SIGPROF",
    "SIGWINCH",
    "SIGINFO",
    "SIGUSR1",
    "SIGUSR2"
};
#endif /* CC_HAVE_SIGNAME */

int
signal_override(int signo, char *info, int flags, uint32_t mask, sig_t handler)
{
    struct sigaction sa;
    int status;
    int i;

    signals[signo].info = info;
    signals[signo].flags = flags;
    signals[signo].handler = handler;
    signals[signo].mask = mask;

    cc_memset(&sa, 0, sizeof(sa));
    sa.sa_flags = signals[signo].flags;
    sa.sa_handler = signals[signo].handler;
    sigemptyset(&sa.sa_mask);
    for (i = SIGNAL_MIN; i < SIGNAL_MAX; ++i) {
        if ( (1 << i) & mask) {
            sigaddset(&sa.sa_mask, i);
        }
    }

    status = sigaction(signo, &sa, NULL);
    if (status < 0) {
        log_error("sigaction(%s) failed: %s", sys_signame[signo],
                  strerror(errno));
    } else {
        log_info("override handler for %s", sys_signame[signo]);
    }

    return status;
}

int
signal_pipe_ignore(void)
{
    return signal_override(SIGPIPE, "ignoring sigpipe (do not exit)", 0, 0,
            SIG_IGN);
}

static void
_handler_stacktrace(int signo)
{
    debug_stacktrace(2); /* skipping functions inside signal module */
    raise(signo);
}

int
signal_segv_stacktrace(void)
{
    return signal_override(SIGSEGV, "printing stacktrace when segfault", 0, 0,
            _handler_stacktrace);
}

static void
_handler_logrotate(int signo)
{
    log_reopen(debug_logger);
}

int
signal_ttin_logrotate(void)
{
    return signal_override(SIGTTIN, "reopen log file", 0, 0, _handler_logrotate);
}
