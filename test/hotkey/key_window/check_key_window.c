#include <hotkey/key_window.h>

#include <hotkey/constant.h>

#include <check.h>

#include <cc_bstring.h>

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define SUITE_NAME "key_window"
#define DEBUG_LOG  SUITE_NAME ".log"

#define TEST_KEY_WINDOW_SIZE 10

/*
 * utilities
 */
static void
test_setup(void)
{
    key_window_setup(TEST_KEY_WINDOW_SIZE);
}

static void
test_teardown(void)
{
    key_window_teardown();
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
    char *key1str = "key1";
    char buf[MAX_KEY_LEN];
    uint32_t key_window_pop_len;
    struct bstring key1;

    key1.data = key1str;
    key1.len = strlen(key1str);

    test_reset();

    ck_assert_int_eq(key_window_len(), 0);

    key_window_push(&key1);
    ck_assert_int_eq(key_window_len(), 1);
    key_window_pop_len = key_window_pop(buf);

    ck_assert_int_eq(key_window_len(), 0);
    ck_assert_int_eq(key_window_pop_len, key1.len);
    ck_assert(strncmp(key1.data, buf, key1.len) == 0);
}
END_TEST

START_TEST(test_multiple)
{
    char *key1str = "key1", *key2str = "key22", *key3str = "key333";
    char buf[MAX_KEY_LEN];
    uint32_t key_window_pop_len;
    struct bstring key1, key2, key3;

    key1.data = key1str;
    key1.len = strlen(key1str);
    key2.data = key2str;
    key2.len = strlen(key2str);
    key3.data = key3str;
    key3.len = strlen(key3str);

    test_reset();

    ck_assert_int_eq(key_window_len(), 0);

    key_window_push(&key1);
    key_window_push(&key2);
    key_window_push(&key3);
    ck_assert_int_eq(key_window_len(), 3);

    key_window_pop_len = key_window_pop(buf);
    ck_assert_int_eq(key_window_pop_len, key1.len);
    ck_assert(strncmp(key1.data, buf, key1.len) == 0);
    ck_assert_int_eq(key_window_len(), 2);

    key_window_pop_len = key_window_pop(buf);
    ck_assert_int_eq(key_window_pop_len, key2.len);
    ck_assert(strncmp(key2.data, buf, key2.len) == 0);
    ck_assert_int_eq(key_window_len(), 1);

    key_window_pop_len = key_window_pop(buf);
    ck_assert_int_eq(key_window_pop_len, key3.len);
    ck_assert(strncmp(key3.data, buf, key3.len) == 0);
    ck_assert_int_eq(key_window_len(), 0);
}
END_TEST

/*
 * test suite
 */
static Suite *
key_window_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic key_window functionality */
    TCase *tc_basic_key_window = tcase_create("basic key_window");
    suite_add_tcase(s, tc_basic_key_window);

    tcase_add_test(tc_basic_key_window, test_basic);
    tcase_add_test(tc_basic_key_window, test_multiple);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = key_window_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VERBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
