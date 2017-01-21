#include <hotkey/queue.h>

#include <hotkey/constant.h>

#include <check.h>

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define SUITE_NAME "queue"
#define DEBUG_LOG  SUITE_NAME ".log"

#define TEST_QUEUE_SIZE 10

/*
 * utilities
 */
static void
test_setup(void)
{
    queue_setup(TEST_QUEUE_SIZE);
}

static void
test_teardown(void)
{
    queue_teardown();
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
    char *key = "key1";
    uint32_t klen = 4;
    char buf[MAX_KEY_LEN];
    uint32_t queue_pop_len;

    test_reset();

    ck_assert_int_eq(queue_len(), 0);

    queue_push(key, klen);
    ck_assert_int_eq(queue_len(), 1);
    queue_pop_len = queue_pop(buf);

    ck_assert_int_eq(queue_len(), 0);
    ck_assert_int_eq(queue_pop_len, klen);
    ck_assert(strncmp(key, buf, klen) == 0);
}
END_TEST

START_TEST(test_multiple)
{
    char *key1 = "key1", *key2 = "key22", *key3 = "key333";
    uint32_t klen1 = 4, klen2 = 5, klen3 = 6;
    char buf[MAX_KEY_LEN];
    uint32_t queue_pop_len;

    test_reset();

    ck_assert_int_eq(queue_len(), 0);

    queue_push(key1, klen1);
    queue_push(key2, klen2);
    queue_push(key3, klen3);
    ck_assert_int_eq(queue_len(), 3);

    queue_pop_len = queue_pop(buf);
    ck_assert_int_eq(queue_pop_len, klen1);
    ck_assert(strncmp(key1, buf, klen1) == 0);
    ck_assert_int_eq(queue_len(), 2);

    queue_pop_len = queue_pop(buf);
    ck_assert_int_eq(queue_pop_len, klen2);
    ck_assert(strncmp(key2, buf, klen2) == 0);
    ck_assert_int_eq(queue_len(), 1);

    queue_pop_len = queue_pop(buf);
    ck_assert_int_eq(queue_pop_len, klen3);
    ck_assert(strncmp(key3, buf, klen3) == 0);
    ck_assert_int_eq(queue_len(), 0);
}
END_TEST

/*
 * test suite
 */
static Suite *
queue_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic queue functionality */
    TCase *tc_basic_queue = tcase_create("basic queue");
    suite_add_tcase(s, tc_basic_queue);

    tcase_add_test(tc_basic_queue, test_basic);
    tcase_add_test(tc_basic_queue, test_multiple);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = queue_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VERBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
