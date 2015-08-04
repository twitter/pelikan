#include <util/log_core.h>

#include <cc_debug.h>
#include <cc_log.h>
#include <cc_mm.h>

#include <string.h>

struct log_core {
    pthread_t thread;
    struct logger *logger;
    int interval;
    bool enable;
};

static void *
log_core_loop(struct log_core *lc)
{
    while (__atomic_load_n(&(lc->enable), __ATOMIC_RELAXED)) {
        usleep(lc->interval);
        log_flush(lc->logger);
    }

    return NULL;
}

struct log_core *
log_core_create(struct logger *logger, int flush_interval)
{
    int status;
    struct log_core *lc = cc_alloc(sizeof(struct log_core));

    if (lc == NULL) {
        log_error("Failed to create log core - out of memory");
        return NULL;
    }

    lc->logger = logger;
    lc->interval = flush_interval;
    lc->enable = true;

    status = pthread_create(&(lc->thread), NULL, (void*(*)(void *))log_core_loop, lc);

    if (status != 0) {
        log_error("Could not create log core: %s", strerror(status));
        cc_free(lc);
        return NULL;
    }

    status = pthread_detach(lc->thread);

    if (status != 0) {
        log_error("Could not detach log core thread: %s", strerror(status));
    }

    return lc;
}

void
log_core_destroy(struct log_core **lc)
{
    if (lc == NULL || *lc == NULL) {
        return;
    }

    __atomic_store_n(&((*lc)->enable), false, __ATOMIC_RELAXED);

    cc_free(*lc);

    *lc = NULL;
}
