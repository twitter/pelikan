#include <time/time.h>

#include <cc_option.h>

#include <check.h>
#include <math.h>
#include <stdlib.h>
#include <time.h>

#define SUITE_NAME "time"
#define DEBUG_LOG  SUITE_NAME ".log"

#define NSEC_PER_USEC  1000L
#define NSEC_PER_MSEC  1000000L
#define NSEC_PER_SEC   1000000000L

time_options_st options = { TIME_OPTION(OPTION_INIT) };

/*
 * utilities
 */
static void
test_setup(void)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    time_setup(&options);
}
static void
test_setup_opt(uint8_t type)
{
    options.time_type.val.vuint = type;
    time_setup(&options);
}

static void
test_teardown(void)
{
    time_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

static void
test_reset_opt(uint8_t type)
{
    test_teardown();
    test_setup_opt(type);
}

/*
 * tests
 */
START_TEST(test_start_time)
{

    test_reset();

    time_update();

    /* check if time_started and time_now are correct on start */
    ck_assert_int_le(labs(time_started() - time(NULL)), 1);
    ck_assert_int_le(labs(time_unix_sec() - time(NULL)), 1);
    ck_assert_int_le(time_proc_sec(), 1);

}
END_TEST

START_TEST(test_unix2proc_sec)
{
#define START                          100
#define NOW                       10000000
#define UTIME_LONG                12345678
#define UTIME_SHORT                    123
#define PTIME_LONG    (UTIME_LONG - START)
#define PTIME_SHORT  (UTIME_SHORT - START)

    test_reset();

    proc_time_i t;

    time_start = START;
    proc_sec = NOW;

    t = time_unix2proc_sec(UTIME_LONG);
    ck_assert_int_eq(t, PTIME_LONG);

    t = time_unix2proc_sec(UTIME_SHORT);
    ck_assert_int_eq(t, PTIME_SHORT);

#undef START
#undef NOW
#undef UTIME_LONG
#undef UTIME_SHORT
#undef PTIME_LONG
#undef PTIME_SHORT
}
END_TEST

START_TEST(test_delta2proc_sec)
{
#define START                          100
#define NOW                       10000000
#define DTIME_LONG                12345678
#define DTIME_SHORT                    123
#define PTIME_LONG      (DTIME_LONG + NOW)
#define PTIME_SHORT    (DTIME_SHORT + NOW)

    test_reset();

    proc_time_i t;

    time_start = START;
    proc_sec = NOW;

    t = time_delta2proc_sec(DTIME_LONG);
    ck_assert_int_eq(t, PTIME_LONG);

    t = time_delta2proc_sec(DTIME_SHORT);
    ck_assert_int_eq(t, PTIME_SHORT);

#undef START
#undef NOW
#undef DTIME_LONG
#undef DTIME_SHORT
#undef PTIME_LONG
#undef PTIME_SHORT
}
END_TEST

START_TEST(test_memcache2proc_sec)
{
#define START                          100
#define NOW                       10000000
#define MTIME_LONG                12345678
#define MTIME_SHORT                    123
#define PTIME_LONG    (MTIME_LONG - START)
#define PTIME_SHORT    (MTIME_SHORT + NOW)

    test_reset();

    proc_time_i t;

    time_start = START;
    proc_sec = NOW;

    t = time_memcache2proc_sec(MTIME_LONG);
    ck_assert_int_eq(t, PTIME_LONG);

    t = time_memcache2proc_sec(MTIME_SHORT);
    ck_assert_int_eq(t, PTIME_SHORT);

#undef START
#undef NOW
#undef MTIME_LONG
#undef MTIME_SHORT
#undef PTIME_LONG
#undef PTIME_SHORT
}
END_TEST

START_TEST(test_convert_proc_sec)
{
#define START                           100
#define NOW                        10000000
#define TIME_LONG                  12345678
#define TIME_SHORT                      123
#define U2P_TIME_LONG   (TIME_LONG - START)
#define U2P_TIME_SHORT (TIME_SHORT - START)
#define D2P_TIME_LONG     (TIME_LONG + NOW)
#define D2P_TIME_SHORT   (TIME_SHORT + NOW)
#define M2P_TIME_LONG   (TIME_LONG - START)
#define M2P_TIME_SHORT   (TIME_SHORT + NOW)

    proc_time_i t;

    test_reset_opt(TIME_UNIX);
    time_start = START;
    proc_sec = NOW;
    t = time_convert_proc_sec(TIME_LONG);
    ck_assert_int_eq(t, U2P_TIME_LONG);
    t = time_convert_proc_sec(TIME_SHORT);
    ck_assert_int_eq(t, U2P_TIME_SHORT);

    test_reset_opt(TIME_DELTA);
    time_start = START;
    proc_sec = NOW;
    t = time_convert_proc_sec(TIME_LONG);
    ck_assert_int_eq(t, D2P_TIME_LONG);
    t = time_convert_proc_sec(TIME_SHORT);
    ck_assert_int_eq(t, D2P_TIME_SHORT);

    test_reset_opt(TIME_MEMCACHE);
    time_start = START;
    proc_sec = NOW;
    t = time_convert_proc_sec(TIME_LONG);
    ck_assert_int_eq(t, M2P_TIME_LONG);
    t = time_convert_proc_sec(TIME_SHORT);
    ck_assert_int_eq(t, M2P_TIME_SHORT);

#undef START
#undef NOW
#undef TIME_LONG
#undef TIME_SHORT
#undef U2P_TIME_LONG
#undef U2P_TIME_SHORT
#undef D2P_TIME_LONG
#undef D2P_TIME_SHORT
#undef M2P_TIME_LONG
#undef M2P_TIME_SHORT
}
END_TEST

START_TEST(test_short_duration)
{
#define DURATION_NS 100000

    proc_time_i s_before, s_after;
    proc_time_fine_i ms_before, ms_after, us_before, us_after, ns_before, ns_after;
    struct timespec ts = (struct timespec){0, DURATION_NS};

    test_reset();

    time_update();
    s_before = time_proc_sec();
    ms_before = time_proc_ms();
    us_before = time_proc_us();
    ns_before = time_proc_ns();

    nanosleep(&ts, NULL);

    time_update();
    s_after = time_proc_sec();
    ms_after = time_proc_ms();
    us_after = time_proc_us();
    ns_after = time_proc_ns();

    /* duration is as expected */
    ck_assert_uint_ge((unsigned int)(ns_after - ns_before), DURATION_NS);
    ck_assert_uint_ge((unsigned int)(us_after - us_before),
            DURATION_NS / NSEC_PER_USEC);
    ck_assert_uint_ge((unsigned int)(ms_after - ms_before),
            DURATION_NS / NSEC_PER_MSEC);
    ck_assert_uint_ge((unsigned int)(s_after - s_before),
            DURATION_NS / NSEC_PER_SEC);

#undef DURATION_NSEC
}
END_TEST

START_TEST(test_long_duration)
{
#define DURATION_S 2

    proc_time_i s_before, s_after;
    proc_time_fine_i ms_before, ms_after, us_before, us_after, ns_before, ns_after;
    struct timespec ts = (struct timespec){DURATION_S, 0};

    test_reset();

    time_update();
    s_before = time_proc_sec();
    ms_before = time_proc_ms();
    us_before = time_proc_us();
    ns_before = time_proc_ns();

    nanosleep(&ts, NULL);

    time_update();
    s_after = time_proc_sec();
    ms_after = time_proc_ms();
    us_after = time_proc_us();
    ns_after = time_proc_ns();

    /* duration is as expected */
    ck_assert_uint_ge((unsigned int)(ns_after - ns_before), DURATION_NS);
    ck_assert_uint_ge((unsigned int)(us_after - us_before),
            DURATION_NS / NSEC_PER_USEC);
    ck_assert_uint_ge((unsigned int)(ms_after - ms_before),
            DURATION_NS / NSEC_PER_MSEC);
    ck_assert_uint_ge((unsigned int)(s_after - s_before),
            DURATION_NS / NSEC_PER_SEC);

#undef DURATION_S
}
END_TEST

/*
 * test suite
 */
static Suite *
time_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_api = tcase_create("time api test");
    suite_add_tcase(s, tc_api);
    tcase_add_test(tc_api, test_start_time);
    tcase_add_test(tc_api, test_unix2proc_sec);
    tcase_add_test(tc_api, test_delta2proc_sec);
    tcase_add_test(tc_api, test_memcache2proc_sec);
    tcase_add_test(tc_api, test_convert_proc_sec);

    TCase *tc_duration = tcase_create("time duration test");
    suite_add_tcase(s, tc_duration);
    tcase_add_test(tc_duration, test_short_duration);
    tcase_add_test(tc_duration, test_long_duration);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = time_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
