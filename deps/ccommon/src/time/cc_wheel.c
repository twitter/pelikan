#include <time/cc_wheel.h>

#include <cc_debug.h>
#include <cc_metric.h>
#include <cc_mm.h>
#include <cc_pool.h>

#include <stdlib.h>

#define TIMING_WHEEL_MODULE_NAME "ccommon::timing_wheel"

FREEPOOL(tevent_pool, teventq, timeout_event);
static struct tevent_pool teventp;
static bool teventp_init = false;

static timing_wheel_metrics_st *timing_wheel_metrics = NULL;
static bool timing_wheel_init = false;

void
timing_wheel_setup(timing_wheel_metrics_st *metrics)
{
    log_info("set up the %s module", TIMING_WHEEL_MODULE_NAME);

    if (timing_wheel_init) {
        log_warn("%s has already been setup, overwrite",
                TIMING_WHEEL_MODULE_NAME);
    }

    timing_wheel_metrics = metrics;
    if (metrics != NULL) {
        TIMING_WHEEL_METRIC_INIT(timing_wheel_metrics);
    }

    timing_wheel_init = true;
}

void
timing_wheel_teardown(void)
{
    log_info("tear down the %s module", TIMING_WHEEL_MODULE_NAME);

    if (!timing_wheel_init) {
        log_warn("%s has never been setup", TIMING_WHEEL_MODULE_NAME);
    }

    timing_wheel_metrics = NULL;
    timing_wheel_init = false;
}

/* timeout_event related functions */

void
timeout_event_reset(struct timeout_event *tev)
{
    ASSERT(tev != NULL);

    STAILQ_NEXT(tev, next) = NULL;
    tev->free = false;

    TAILQ_NEXT(tev, tqe) = NULL;
    TAILQ_PREV(tev, tevent_tqh, tqe) = NULL;
    tev->cb = NULL;
    tev->data = NULL;
    tev->recur = false;
    timeout_reset(&tev->delay);
    /* members used by timing wheel are set/cleared by timing wheel ops */
}

struct timeout_event *
timeout_event_create(void)
{
    struct timeout_event *t = (struct timeout_event *)cc_alloc(sizeof(*t));
    if (t == NULL) {
        log_info("timeout_event creation failed due to OOM");
        INCR(timing_wheel_metrics, timeout_event_create_ex);

        return NULL;
    }

    timeout_event_reset(t);
    INCR(timing_wheel_metrics, timeout_event_create);
    INCR(timing_wheel_metrics, timeout_event_curr);
    log_verb("created timeout_event %p", t);

    return t;
}

void
timeout_event_destroy(struct timeout_event **tev)
{
    struct timeout_event *t = *tev;

    if (t == NULL) {
        return;
    }

    log_verb("destroy timeout_event %p", t);

    cc_free(t);
    *tev = NULL;
    INCR(timing_wheel_metrics, timeout_event_destroy);
    DECR(timing_wheel_metrics, timeout_event_curr);
}

struct timeout_event *
timeout_event_borrow(void)
{
    struct timeout_event *t;

    FREEPOOL_BORROW(t, &teventp, next, timeout_event_create);

    if (t == NULL) {
        log_debug("borrow timeout_event failed: OOM or over limit");
        INCR(timing_wheel_metrics, timeout_event_borrow_ex);

        return NULL;
    }

    timeout_event_reset(t);
    INCR(timing_wheel_metrics, timeout_event_borrow);
    INCR(timing_wheel_metrics, timeout_event_active);

    log_verb("borrow timeout_event %p", t);

    return t;
}

void
timeout_event_return(struct timeout_event **tev)
{
    struct timeout_event *t = *tev;

    if (t == NULL || t->free) {
        return;
    }

    log_verb("return timeout_event %p", t);

    t->free = true;
    FREEPOOL_RETURN(&teventp, t, next);

    *tev = NULL;
    INCR(timing_wheel_metrics, timeout_event_return);
    DECR(timing_wheel_metrics, timeout_event_active);
}

void
timeout_event_pool_create(uint32_t max)
{
    struct timeout_event *t;

    if (teventp_init) {
        log_warn("timeout_event pool has already been created, ignore");

        return;
    }

    log_info("creating timeout_event pool: max %"PRIu32, max);

    FREEPOOL_CREATE(&teventp, max);
    teventp_init = true;

    /* preallocating, see notes in buffer/cc_buf.c */

    if (max == 0) { /* do not preallocate if pool size is not specified */
        return;
    }

    FREEPOOL_PREALLOC(t, &teventp, max, next, timeout_event_create);
    if (t == NULL) {
        log_crit("cannot preallocate timeout_event pool due to OOM, abort");
        exit(EXIT_FAILURE);
    }
}

void
timeout_event_pool_destroy(void)
{
    struct timeout_event *t, *tt;

    if (teventp_init) {
        log_warn("timeout_event pool was never created, ignore");

        return;
    }

    log_info("destroying timeout_event pool: free %"PRIu32, teventp.nfree);

    FREEPOOL_DESTROY(t, tt, &teventp, next, timeout_event_destroy);
    teventp_init = false;
}


/* timing wheel related functions */

struct timing_wheel *
timing_wheel_create(struct timeout *tick, size_t cap, size_t ntick)
{
    struct timing_wheel *tw = (struct timing_wheel *)cc_alloc(sizeof(*tw));

    ASSERT(tick != NULL);
    ASSERT(cap > 0 && ntick > 0);

    if (tw == NULL) {
        log_error("timing_wheel creation failed due to OOM");

        return NULL;
    }

    tw->tick = *tick;
    tw->tick_ns = timeout_ns(tick);
    tw->cap = cap;
    tw->max_ntick = ntick;
    tw->active = false;
    timeout_reset(&tw->due);
    tw->curr = 0;
    tw->nevent = 0;

    tw->table = (struct tevent_tqh *)cc_alloc(cap * sizeof(struct tevent_tqh));
    if (tw->table == NULL) {
        log_error("timing_wheel creation failed due to table allocation OOM");
        cc_free(tw);

        return NULL;
    }
    for (size_t i = 0; i < cap; i++) {
        TAILQ_INIT(&tw->table[i]);
    }

    tw->nprocess = 0;
    tw->ntick = 0;
    tw->nexec = 0;

    log_info("created timing_wheel %p", tw);

    return tw;
}

void
timing_wheel_destroy(struct timing_wheel **tw)
{
    struct timing_wheel *w = *tw;

    log_info("destroying timing_wheel %p", w);

    cc_free(w->table);
    cc_free(w);

    *tw = NULL;
}

/**
 * Since timing wheel is discrete, the events are bucket'ed approximately.
 * Here we treat ms == 0 as a special case and add event to the current slot,
 * otherwise, the offset is at least 1 (next slot)
 */
rstatus_i
timing_wheel_insert(struct timing_wheel *tw, struct timeout_event *tev)
{
    size_t offset = 0;

    ASSERT(tw != NULL && tev != NULL);
    ASSERT(tev->delay.is_intvl);

    tev->delay_ns = (uint64_t)timeout_ns(&tev->delay);

    if (tev->delay_ns > 0) {
        offset = (tev->delay_ns - 1) / tw->tick_ns + 1;
    }

    if (offset >= tw->cap) { /* wraps around */
        log_error("insert timeout event into timing wheel failed: timeout "
                PRIu64"ns too long for wheel capacity %"PRIu64"ns",
                tev->delay_ns, tw->tick_ns * tw->cap);

        return CC_EINVAL;
    }

    if (tev->recur && tev->delay_ns == 0) {
        log_error("insert timeout event into timing wheel failed: timeout "
                "cannot be 0 for recurring events");

        return CC_EINVAL;
    }

    timeout_add_intvl(&tev->to, &tev->delay);
    offset = (tw->curr + offset) % tw->cap; /* convert to absolute offset */
    tev->offset = offset;
    TAILQ_INSERT_TAIL(&tw->table[offset], tev, tqe);
    tw->nevent++;
    INCR(timing_wheel_metrics, timing_wheel_insert);
    INCR(timing_wheel_metrics, timing_wheel_event);

    log_verb("added timeout event %p into timing wheel %p: curr tick %zu, "
            "scheduled offset %zu", tev, tw, tw->curr, tev->offset);

    return CC_OK;
}

void
timing_wheel_remove(struct timing_wheel *tw, struct timeout_event *tev)
{
    ASSERT(tw != NULL && tev != NULL);

    log_verb("removing timeout event %p from timing wheel %p: curr tick %zu, "
            "scheduled offset %zu", tev, tw, tw->curr, tev->offset);

    TAILQ_REMOVE(&tw->table[tev->offset], tev, tqe);
    tev->offset = 0;
    timeout_reset(&tev->to);
    tev->delay_ns = 0;
    tw->nevent--;
    INCR(timing_wheel_metrics, timing_wheel_remove);
    DECR(timing_wheel_metrics, timing_wheel_event);
}

void
timing_wheel_start(struct timing_wheel *tw)
{
    /* when timing wheel is created, `due' is reset with `is_set' set to false
     * (what a tongue-twister...), so timeout_expired always returns false for
     * `due', and timing_wheel_execute won't fire any timeout events inserted.
     *
     * calling this function sets due to a valid timestamp in the future, and
     * the wheel starts turning...
     */
    log_info("starting timing wheel %p", tw);

    tw->active = true;
    timeout_add_intvl(&tw->due, &tw->tick);
}

void
timing_wheel_stop(struct timing_wheel *tw)
{
    /* turn `is_set' to false for `due' so timeout_expired always returns false,
     * and timing_wheel_execute won't fire any timeout events inserted.
     */
    log_info("stopping timing wheel %p", tw);

    tw->active = false;
    timeout_reset(&tw->due);
}

static inline void
_advance_curr(struct timing_wheel *tw)
{
    log_verb("advancing the current tick of timing wheel %p from %zu", tw,
            tw->curr);

    tw->curr++;
    tw->curr %= tw->cap;

    tw->ntick++;
    INCR(timing_wheel_metrics, timing_wheel_tick);
}

static inline void
_process_tick(struct timing_wheel *tw, bool endmode)
{
    struct timeout_event *t, *tt;
    rstatus_i status;
    uint64_t nprocess = tw->nprocess;

    TAILQ_FOREACH_SAFE(t, &tw->table[tw->curr], tqe, tt) {
        tw->nprocess++;
        INCR(timing_wheel_metrics, timing_wheel_process);

        t->cb(t->data);
        timing_wheel_remove(tw, t);
        if (!endmode && t->recur) { /* reinsert if recurring and not ending */
            status = timing_wheel_insert(tw, t);
            ASSERT(status == CC_OK); /* shouldn't fail */
        }
    }

    log_verb("processed %"PRIu64" timeout events during tick %zu of timing "
            "wheel %p", tw->nprocess - nprocess, tw->curr, tw);
}

void
timing_wheel_execute(struct timing_wheel *tw)
{
    ASSERT(tw != NULL);
    size_t ntick = 0;
    uint64_t elapsed = 0;


    /*
     * If timing wheel's current slot is not due, it returns immediately;
     * if multiple slots are due, they will all be triggered in one func call.
     *
     * This allows the execution to be called anytime to opportunistically
     * trigger all the timers expired. For example, an application can check
     * if there's any timeouts after every N requests. Separating timing wheel
     * execution from the clock means an innate clock or wait mechanism is not
     * dictated by the wheel, and user can choose any mechanisms to advance the
     * clock, e.g. nanosleep, select, epoll_wait/kqueue...
     */
    while (ntick < tw->max_ntick && timeout_expired(&tw->due)) {
        struct duration d;
        struct timeout to;

        duration_start(&d);

        ntick++;
        _process_tick(tw, false);
        _advance_curr(tw);

        duration_stop(&d);
        elapsed += duration_ns(&d);
        timeout_set_ns(&to, duration_ns(&d));
        /* add a tick, and subtract elapsed time */
        if (to.tp < tw->tick.tp) { /* avoid due timestamp regression */
            timeout_sum_intvl(&tw->due, &tw->due, &tw->tick);
            timeout_sub_intvl(&tw->due, &tw->due, &to);
        }
    }

    log_verb("execution round %"PRIu64" processed %zu ticks of timing wheel %p "
            "in %"PRIu64" ns", tw->nexec, ntick, tw, elapsed);

    tw->nexec++;
    INCR(timing_wheel_metrics, timing_wheel_exec);
}

void
timing_wheel_flush(struct timing_wheel *tw)
{
    ASSERT(tw != NULL);

    size_t start = tw->curr;

    log_verb("flushing all remaining ticks in timing wheel %p", tw);

    do {
        _process_tick(tw, true);
        _advance_curr(tw);
    } while (tw->curr != start);
}
