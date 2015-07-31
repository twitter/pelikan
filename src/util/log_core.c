#include <util/log_core.h>

#include <cc_debug.h>
#include <cc_log.h>
#include <cc_mm.h>

#include <pthread.h>
#include <string.h>

struct log_core {
    pthread_t thread;
    struct logger *logger;
    int interval;
};

static void *
log_core_loop(void *args)
{
    struct logger *logger = ((struct log_core *)args)->logger;
    int interval = ((struct log_core *)args)->interval;
    pthread_t thread = ((struct log_core *)args)->thread;

    cc_free(args);

    for (;;) {
        usleep(interval);
        log_flush(logger);
    }

    return NULL;
}

rstatus_t
log_core_create(struct logger *logger, int flush_interval)
{
    int status;
    struct log_core *args = cc_alloc(sizeof(struct log_core));

    if (args == NULL) {
        log_error("Failed to create log core - out of memory");
        return CC_ENOMEM;
    }

    args->logger = logger;
    args->interval = flush_interval;

    status = pthread_create(&(args->thread), NULL, log_core_loop, args);

    if (status != 0) {
        log_error("Could not create log core: %s", strerror(status));
        return CC_ERROR;
    }

    return CC_OK;
}
