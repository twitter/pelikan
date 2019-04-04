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

/*
 * tests
 */
START_TEST(test_datapool)
{
    int fresh = 0;
	struct datapool *pool = datapool_open(TEST_DATAFILE, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(TEST_DATAFILE, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    ck_assert_int_eq(s, datapool_size(pool));
    ck_assert_int_eq(fresh, 0);
    datapool_close(pool);
}
END_TEST

START_TEST(test_devzero)
{
    int fresh = 0;
	struct datapool *pool = datapool_open(NULL, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    size_t s = datapool_size(pool);
    ck_assert_int_ge(s, TEST_DATASIZE);
    ck_assert_int_eq(fresh, 1);
    ck_assert_ptr_nonnull(datapool_addr(pool));
    datapool_close(pool);

    pool = datapool_open(NULL, TEST_DATASIZE, &fresh);
    ck_assert_ptr_nonnull(pool);
    ck_assert_int_eq(s, datapool_size(pool));
    ck_assert_int_eq(fresh, 1);
    datapool_close(pool);
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

    unlink(TEST_DATAFILE);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
