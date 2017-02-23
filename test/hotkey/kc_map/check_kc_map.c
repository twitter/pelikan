#include <hotkey/kc_map.h>

#include <hotkey/constant.h>

#include <check.h>

#include <cc_bstring.h>

#include <stdlib.h>
#include <string.h>

#define SUITE_NAME "kc_map"
#define DEBUG_LOG  SUITE_NAME ".log"

#define TEST_TABLE_SIZE 10

/*
 * utilities
 */
static void
test_setup(void)
{
    kc_map_setup(TEST_TABLE_SIZE, TEST_TABLE_SIZE);
}

static void
test_teardown(void)
{
    kc_map_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

/**************
 * test cases *
 **************/

START_TEST(test_basic)
{
#define KEY1 "key1"
#define KEY2 "key22"
    uint32_t count;
    struct bstring key1 = str2bstr(KEY1), key2 = str2bstr(KEY2);

    test_reset();

    count = kc_map_incr(&key1);
    ck_assert_int_eq(count, 1);

    count = kc_map_incr(&key2);
    ck_assert_int_eq(count, 1);

    count = kc_map_incr(&key1);
    ck_assert_int_eq(count, 2);

    kc_map_decr(&key1);
    count = kc_map_incr(&key1);
    ck_assert_int_eq(count, 2);
#undef KEY1
#undef KEY2
}
END_TEST

/*
 * test suite
 */
static Suite *
kc_map_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic functionality */
    TCase *tc_basic_kc_map = tcase_create("basic kc_map");
    suite_add_tcase(s, tc_basic_kc_map);

    tcase_add_test(tc_basic_kc_map, test_basic);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = kc_map_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VERBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
