#include <time/cc_wheel.h>

#include <check.h>

#include <stdlib.h>
#include <stdio.h>
#include <time.h>

#define SUITE_NAME "wheel"
#define DEBUG_LOG  SUITE_NAME ".log"

static timing_wheel_metrics_st metrics;

/*
 * utilities
 */
static void
test_setup(void)
{
    metrics = (timing_wheel_metrics_st) { TIMING_WHEEL_METRIC(METRIC_INIT) };
    timing_wheel_setup(&metrics);
}

static void
test_teardown(void)
{
    timing_wheel_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

/*
 * tests
 */
START_TEST(test_timeout_event_create_destroy)
{
    struct timeout_event *tev, *tev2;

    test_reset();

    tev = timeout_event_create();
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 1);
    timeout_event_destroy(&tev);
    ck_assert_uint_eq(metrics.timeout_event_destroy.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 0);

    tev = timeout_event_create();
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 2);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 1);
    tev2 = timeout_event_create();
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 3);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 2);
    timeout_event_destroy(&tev);
    timeout_event_destroy(&tev2);
    ck_assert_uint_eq(metrics.timeout_event_destroy.counter, 3);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 0);
}
END_TEST

START_TEST(test_timeout_event_pool)
{
#define POOL_SIZE 2

    struct timeout_event *tev, *tev2;

    test_reset();

    timeout_event_pool_create(POOL_SIZE);
    ck_assert_uint_eq(metrics.timeout_event_create.counter, POOL_SIZE);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, POOL_SIZE);

    tev = timeout_event_borrow();
    ck_assert_uint_eq(metrics.timeout_event_borrow.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 1);
    timeout_event_return(&tev);
    ck_assert_uint_eq(metrics.timeout_event_return.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 0);

    tev = timeout_event_borrow();
    ck_assert_uint_eq(metrics.timeout_event_borrow.counter, 2);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 1);
    tev2 = timeout_event_borrow();
    ck_assert_uint_eq(metrics.timeout_event_borrow.counter, 3);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 2);
    ck_assert(timeout_event_borrow() == NULL); /* over pool limit */
    ck_assert_uint_eq(metrics.timeout_event_borrow_ex.counter, 1);
    timeout_event_return(&tev);
    timeout_event_return(&tev2);
    ck_assert_uint_eq(metrics.timeout_event_return.counter, 3);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 0);

    timeout_event_pool_destroy();
    ck_assert_uint_eq(metrics.timeout_event_destroy.counter, POOL_SIZE);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 0);

#undef POOL_SIZE
}
END_TEST

START_TEST(test_timeout_event_pool_unlimited)
{
    struct timeout_event *tev;

    test_reset();

    timeout_event_pool_create(0);
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 0);

    tev = timeout_event_borrow();
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_borrow.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 1);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 1);
    timeout_event_return(&tev);
    ck_assert_uint_eq(metrics.timeout_event_return.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 0);

    timeout_event_pool_destroy();
    ck_assert_uint_eq(metrics.timeout_event_destroy.counter, 1);
    ck_assert_uint_eq(metrics.timeout_event_curr.gauge, 0);
}
END_TEST

START_TEST(test_timeout_event_edge_case)
{
    struct timeout_event *tev = NULL;

    test_reset();

    /* destroy edge cases */
    timeout_event_destroy(NULL);
    timeout_event_destroy(&tev);

    /* return edge cases */
    timeout_event_return(NULL);
    timeout_event_return(&tev);

    /* pool destroy re-entry should be fine */
    timeout_event_pool_destroy();
    timeout_event_pool_destroy();

    /* create re-entry should be fine */
    timeout_event_pool_create(0);
    timeout_event_pool_create(0);

    tev = timeout_event_borrow();
    tev->free = true;
    timeout_event_return(&tev); /* should not return an already free item */
    ck_assert_uint_eq(metrics.timeout_event_active.gauge, 1);

}
END_TEST

static void
_incr_cb(void *v)
{
    *(int *)v += 1;
}

START_TEST(test_timing_wheel_basic)
{
#define TICK_NS 1000000
#define NSLOT 3
#define NTICK 2

    struct timeout tick, delay;
    struct timing_wheel *tw;
    struct timeout_event *tev;
    struct timespec short_ts = (struct timespec){0, TICK_NS * NTICK};
    struct timespec long_ts = (struct timespec){0, TICK_NS * (NTICK + 1)};
    int i = 0;

    test_reset();

    timeout_set_ns(&tick, TICK_NS);
    timeout_set_ns(&delay, TICK_NS * 3 / 2); /* between 1 & 2 ticks */

    /* init & start timing wheel */
    tw = timing_wheel_create(&tick, NSLOT, NTICK);
    timing_wheel_start(tw);
    ck_assert_int_le(timeout_ns(&tw->due), TICK_NS);

    /* init, insert, delete timeout event */
    tev = timeout_event_create();
    tev->cb = _incr_cb;
    tev->data = &i;
    tev->recur = false;
    tev->delay = delay;

    timing_wheel_insert(tw, tev);
    ck_assert_int_eq(tw->nevent, 1);
    timing_wheel_remove(tw, tev);
    ck_assert_int_eq(tw->nevent, 0);

    /* execute with finer clock */
    timing_wheel_insert(tw, tev);
    ck_assert_int_eq(tw->nevent, 1);
    nanosleep(&short_ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nexec, 1);
    ck_assert_int_ge(tw->ntick, 1);
    nanosleep(&short_ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nexec, 2);
    ck_assert_int_ge(tw->ntick, 2);
    ck_assert_int_eq(tw->nprocess, 1);
    ck_assert_int_eq(i, 1);

    /* execute with coarser clock/sleep */
    timing_wheel_insert(tw, tev);
    nanosleep(&long_ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nexec, 3);
    ck_assert_int_ge(tw->ntick, 2 + NTICK);
    ck_assert_int_eq(tw->nprocess, 1); /* limited by ntick */
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nexec, 4);
    ck_assert_int_ge(tw->ntick, 3 + NTICK);
    ck_assert_int_eq(tw->nprocess, 2);
    ck_assert_int_eq(i, 2);

    /* add to the immediate next tick */
    timeout_set_ns(&delay, 0);
    tev->delay = delay;
    timing_wheel_insert(tw, tev);
    nanosleep(&short_ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nprocess, 3);

    timing_wheel_stop(tw);
    timeout_event_destroy(&tev);
    timing_wheel_destroy(&tw);

#undef NTICK
#undef NSLOT
#undef TICK_MS
}
END_TEST

START_TEST(test_timing_wheel_recur)
{
#define TICK_NS 1000000
#define NSLOT 3
#define NTICK 2

    struct timeout tick, delay;
    struct timing_wheel *tw;
    struct timeout_event *tev;
    struct timespec ts = (struct timespec){0, TICK_NS};
    int i = 0;

    test_reset();

    timeout_set_ns(&tick, TICK_NS);
    timeout_set_ns(&delay, TICK_NS / 2);

    tw = timing_wheel_create(&tick, NSLOT, NTICK);
    timing_wheel_start(tw);
    ck_assert_int_le(timeout_ns(&tw->due), TICK_NS);

    tev = timeout_event_create();
    tev->cb = _incr_cb;
    tev->data = &i;
    tev->recur = true;
    tev->delay = delay;
    timing_wheel_insert(tw, tev);

    nanosleep(&ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nprocess, 0);
    ck_assert_int_eq(tw->nevent, 1);

    nanosleep(&ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nevent, 1);
    ck_assert_int_eq(tw->nprocess, 1);
    ck_assert_int_eq(tw->nevent, 1);
    ck_assert_int_eq(i, 1);
    nanosleep(&ts, NULL);
    timing_wheel_execute(tw);
    ck_assert_int_eq(tw->nprocess, 2);
    ck_assert_int_eq(i, 2);

    timing_wheel_stop(tw);
    timing_wheel_flush(tw);
    ck_assert_int_eq(tw->nevent, 0);
    ck_assert_int_eq(tw->nprocess, 3);
    timeout_event_destroy(&tev);
    timing_wheel_destroy(&tw);

#undef NTICK
#undef NSLOT
#undef TICK_MS
}
END_TEST

START_TEST(test_timing_wheel_edge_case)
{
#define TICK_NS 1000000
#define NSLOT 3
#define NTICK 2

    struct timeout tick, delay;
    struct timing_wheel *tw;
    struct timeout_event *tev;

    /* re-entry on teardown should work */
    timing_wheel_teardown();
    timing_wheel_teardown();

    /* re-entry on setup should work */
    metrics.timeout_event_create.counter = 1;
    timing_wheel_setup(NULL);
    timing_wheel_setup(&metrics);
    ck_assert_uint_eq(metrics.timeout_event_create.counter, 1);

    timeout_set_ns(&tick, TICK_NS);
    timeout_set_ns(&delay, TICK_NS * NSLOT);
    tw = timing_wheel_create(&tick, NSLOT, NTICK);
    timing_wheel_start(tw);

    tev = timeout_event_create();
    tev->recur = true;
    timeout_set_ns(&tev->delay, 0);
    ck_assert(timing_wheel_insert(tw, tev) == CC_EINVAL);
    tev->delay = delay;
    ck_assert(timing_wheel_insert(tw, tev) == CC_EINVAL);

    timing_wheel_destroy(&tw);

#undef NTICK
#undef NSLOT
#undef TICK_MS
}
END_TEST


/*
 * test suite
 */
static Suite *
wheel_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_wheel = tcase_create("timer/timing_wheel test");
    suite_add_tcase(s, tc_wheel);

    tcase_add_test(tc_wheel, test_timeout_event_create_destroy);
    tcase_add_test(tc_wheel, test_timeout_event_pool);
    tcase_add_test(tc_wheel, test_timeout_event_pool_unlimited);
    tcase_add_test(tc_wheel, test_timeout_event_edge_case);
    tcase_add_test(tc_wheel, test_timing_wheel_basic);
    tcase_add_test(tc_wheel, test_timing_wheel_recur);
    tcase_add_test(tc_wheel, test_timing_wheel_edge_case);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = wheel_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
