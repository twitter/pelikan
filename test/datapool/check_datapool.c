#include <datapool/datapool.h>

#include <cc_option.h>

#include <check.h>
#include <math.h>
#include <stdlib.h>
#include <time.h>

#define SUITE_NAME "datapool"
#define DEBUG_LOG  SUITE_NAME ".log"
#define TEST_DATAFILE "./datapool.pelikan"
#define TEST_DATASIZE (1 << 20)
#define TEST_DATA_NAME "datapool_pelikan"

static void
test_teardown(const char* str)
{
    unlink(str);
}

/*
 * tests
 */
START_TEST(test_datapool)
{
    int fresh = 0;
    struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    ck_assert_int_eq(s, datapool_size(pool));
    ck_assert_int_eq(fresh, 0);
    datapool_close(pool);
    test_teardown(TEST_DATAFILE);
}
END_TEST

START_TEST(test_devzero)
{
    int fresh = 0;
    struct datapool *pool = datapool_open(NULL, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(NULL, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    ck_assert_int_eq(s, datapool_size(pool));
    ck_assert_int_eq(fresh, 1);
    datapool_close(pool);
    test_teardown(TEST_DATAFILE);
}
END_TEST

START_TEST(test_datapool_userdata)
{
#define MAX_USER_DATA_SIZE 2000
    char data_set[MAX_USER_DATA_SIZE] = {0};
    char data_get[MAX_USER_DATA_SIZE] = {0};

    struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, NULL);
    ck_assert_ptr_nonnull(pool);
    cc_memset(data_set, 'A', MAX_USER_DATA_SIZE);
    datapool_set_user_data(pool, data_set, MAX_USER_DATA_SIZE);
    datapool_close(pool);

    pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, NULL);
    ck_assert_ptr_nonnull(pool);
    datapool_get_user_data(pool, data_get, MAX_USER_DATA_SIZE);
    ck_assert_mem_eq(data_set, data_get, MAX_USER_DATA_SIZE);
    datapool_close(pool);
    test_teardown(TEST_DATAFILE);
#undef MAX_USER_DATA_SIZE
}
END_TEST


START_TEST(test_datapool_prealloc)
{
    struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, NULL, true);
    ck_assert_ptr_nonnull(pool);
    datapool_close(pool);
    test_teardown(TEST_DATAFILE);
}
END_TEST

START_TEST(test_datapool_empty_signature)
{
    struct datapool *pool = datapool_open(TEST_DATAFILE, NULL, TEST_DATASIZE, NULL);
    ck_assert_ptr_null(pool);
}
END_TEST

START_TEST(test_datapool_too_long_signature)
{
#define LONG_SIGNATURE "Lorem ipsum dolor sit amet, consectetur volutpat"
    struct datapool *pool = datapool_open(TEST_DATAFILE, LONG_SIGNATURE, TEST_DATASIZE, NULL);
    ck_assert_ptr_null(pool);
#undef LONG_SIGNATURE
}
END_TEST

START_TEST(test_datapool_max_length_signature)
{
#define MAX_SIGNATURE "Lorem ipsum dolor sit amet, consectetur volutpa"
    struct datapool *pool = datapool_open(TEST_DATAFILE, MAX_SIGNATURE, TEST_DATASIZE, NULL, false);
    ck_assert_ptr_nonnull(pool);
    datapool_close(pool);
    test_teardown(TEST_DATAFILE);
#undef MAX_SIGNATURE
}
END_TEST

START_TEST(test_datapool_wrong_signature_long_variant)
{
#define WRONG_POOL_NAME_LONG_VAR "datapool_pelikan_no_exist"
    int fresh = 0;
    struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(TEST_DATAFILE, WRONG_POOL_NAME_LONG_VAR, TEST_DATASIZE, NULL);
    ck_assert_ptr_null(pool);
    test_teardown(TEST_DATAFILE);
#undef WRONG_POOL_NAME_LONG_VAR
}
END_TEST

START_TEST(test_datapool_wrong_signature_short_variant)
{
#define WRONG_POOL_NAME_SHORT_VAR "datapool"
    int fresh = 0;
    struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATA_NAME, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(TEST_DATAFILE, WRONG_POOL_NAME_SHORT_VAR, TEST_DATASIZE, NULL);
    ck_assert_ptr_null(pool);
    test_teardown(TEST_DATAFILE);
#undef WRONG_POOL_NAME_SHORT_VAR
}
END_TEST

/*
 * test suite
 */
static Suite *
datapool_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_pool = tcase_create("pool");
    tcase_add_test(tc_pool, test_datapool);
    tcase_add_test(tc_pool, test_devzero);
    tcase_add_test(tc_pool, test_datapool_userdata);
    tcase_add_test(tc_pool, test_datapool_max_length_signature);
    tcase_add_test(tc_pool, test_datapool_empty_signature);
    tcase_add_test(tc_pool, test_datapool_too_long_signature);
    tcase_add_test(tc_pool, test_datapool_wrong_signature_short_variant);
    tcase_add_test(tc_pool, test_datapool_wrong_signature_long_variant);

    suite_add_tcase(s, tc_pool);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = datapool_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_fork_status(srunner, CK_NOFORK);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
