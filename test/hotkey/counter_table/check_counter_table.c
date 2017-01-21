#include <hotkey/counter_table.h>

#include <hotkey/constant.h>

#include <check.h>

#include <stdlib.h>

#define SUITE_NAME "counter_table"
#define DEBUG_LOG  SUITE_NAME ".log"

#define TEST_TABLE_SIZE 10

/*
 * utilities
 */
static void
test_setup(void)
{
    counter_table_setup(TEST_TABLE_SIZE, TEST_TABLE_SIZE);
}

static void
test_teardown(void)
{
    counter_table_teardown();
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
    char *key1 = "key1", *key2 = "key2";
    uint32_t klen = 4;
    uint32_t count;

    test_reset();

    count = counter_table_incr(key1, klen);
    ck_assert_int_eq(count, 1);

    count = counter_table_incr(key2, klen);
    ck_assert_int_eq(count, 1);

    count = counter_table_incr(key1, klen);
    ck_assert_int_eq(count, 2);

    counter_table_decr(key1, klen);
    count = counter_table_incr(key1, klen);
    ck_assert_int_eq(count, 2);
}
END_TEST

/*
 * test suite
 */
static Suite *
counter_table_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic functionality */
    TCase *tc_basic_counter_table = tcase_create("basic counter_table");
    suite_add_tcase(s, tc_basic_counter_table);

    tcase_add_test(tc_basic_counter_table, test_basic);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = counter_table_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VERBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
