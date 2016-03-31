#include <time/cc_timer.h>

#include <check.h>

#include <inttypes.h>
#include <math.h>
#include <stdlib.h>
#include <stdio.h>
#include <time.h>

#define SUITE_NAME "timer"
#define DEBUG_LOG  SUITE_NAME ".log"

/*
 * utilities
 */
static void
test_setup(void)
{
}

static void
test_teardown(void)
{
}

/*
 * tests
 */
START_TEST(test_duration)
{
#define DURATION_NS 100000

    struct duration d;
    double d_ns, d_us, d_ms, d_sec;
    struct timespec ts = (struct timespec){0, DURATION_NS};

    duration_reset(&d);
    duration_start(&d);
    nanosleep(&ts, NULL);
    duration_stop(&d);

    /* duration is as expected */
    d_ns = duration_ns(&d);
    ck_assert_uint_ge((unsigned int)d_ns, DURATION_NS);

    /* readings of different units are consistent */
    d_us = duration_us(&d);
    d_ms = duration_ms(&d);
    d_sec = duration_sec(&d);
    ck_assert(fabs(d_ns - d_us * 1000) < 1e-5);
    ck_assert(fabs(d_us - d_ms * 1000) < 1e-5);
    ck_assert(fabs(d_ms - d_sec * 1000) < 1e-5);

#undef DURATION_NS
}
END_TEST

START_TEST(test_timeout_intvl)
{
#define INTVL_SEC 2

    struct timeout e, f;
    struct timespec ts;

    timeout_reset(&e);
    timeout_reset(&f);

    /* reading the same interval */
    timeout_set_sec(&e, INTVL_SEC);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC);
    ck_assert_int_eq(timeout_ms(&e), timeout_sec(&e) * 1000);
    ck_assert_int_eq(timeout_us(&e), timeout_ms(&e) * 1000);
    ck_assert_int_eq(timeout_ns(&e), timeout_us(&e) * 1000);
    timeout_timespec(&ts, &e);
    ck_assert_int_eq(ts.tv_sec, INTVL_SEC);
    ck_assert_int_eq(ts.tv_nsec, 0);

    /* setting the same interval */
    timeout_set_ms(&e, timeout_sec(&e) * 1000);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC);
    timeout_set_us(&e, timeout_ms(&e) * 1000);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC);
    timeout_set_ns(&e, timeout_us(&e) * 1000);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC);

    /* interval sum/sub */
    timeout_set_sec(&f, INTVL_SEC);
    timeout_sum_intvl(&e, &e, &f);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC + INTVL_SEC);
    timeout_sub_intvl(&e, &e, &f);
    ck_assert_int_eq(timeout_sec(&e), INTVL_SEC);

#undef INTVL_SEC
}
END_TEST

START_TEST(test_timeout_absolute)
{
#define TIMEOUT_NS 100000

    struct timeout e, f;
    struct timespec ts = (struct timespec){0, TIMEOUT_NS};

    timeout_reset(&e);
    ck_assert(!timeout_expired(&e));
    timeout_reset(&f);
    timeout_set_ns(&f, TIMEOUT_NS);

    /* add timeout and sleep: ns, us, intvl */
    timeout_add_ns(&e, TIMEOUT_NS);
    ck_assert(!timeout_expired(&e));
    ck_assert_int_le(timeout_ns(&e), TIMEOUT_NS);
    nanosleep(&ts, NULL);
    ck_assert(timeout_expired(&e));

    timeout_add_us(&e, TIMEOUT_NS / 1000);
    ck_assert(!timeout_expired(&e));
    ck_assert_int_le(timeout_us(&e), TIMEOUT_NS / 1000);
    nanosleep(&ts, NULL);
    ck_assert(timeout_expired(&e));

    timeout_add_intvl(&e, &f);
    ck_assert(!timeout_expired(&e));
    nanosleep(&ts, NULL);
    ck_assert(timeout_expired(&e));

    /* add timeout only: ms, sec */
    timeout_add_ms(&e, 0);
    ck_assert(timeout_expired(&e));
    ck_assert_int_le(timeout_ms(&e), 0);
    timeout_add_ms(&e, 1);
    ck_assert(!timeout_expired(&e));
    ck_assert_int_le(timeout_ms(&e), 1);

    timeout_add_sec(&e, 0);
    ck_assert(timeout_expired(&e));
    ck_assert_int_le(timeout_sec(&e), 0);
    timeout_add_sec(&e, 1);
    ck_assert(!timeout_expired(&e));
    ck_assert_int_le(timeout_sec(&e), 1);

    /* interval sub/sub */
    timeout_reset(&e);
    timeout_add_ns(&e, 0);
    timeout_sum_intvl(&e, &e, &f);
    ck_assert(!timeout_expired(&e));
    ck_assert_int_le(timeout_ns(&e), TIMEOUT_NS);

    timeout_reset(&e);
    timeout_add_ns(&e, 0);
    timeout_sub_intvl(&e, &e, &f);
    ck_assert(timeout_expired(&e));
    ck_assert_int_le(timeout_ns(&e), -TIMEOUT_NS);

#undef TIMEOUT_NS
}
END_TEST


/*
 * test suite
 */
static Suite *
timer_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* duration */
    TCase *tc_duration = tcase_create("timer/duration test");
    suite_add_tcase(s, tc_duration);

    tcase_add_test(tc_duration, test_duration);

    /* timeout */
    TCase *tc_timeout = tcase_create("timer/timeout test");
    suite_add_tcase(s, tc_timeout);

    tcase_add_test(tc_timeout, test_timeout_intvl);
    tcase_add_test(tc_timeout, test_timeout_absolute);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = timer_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
