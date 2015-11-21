#include <cc_bstring.h>

#include <check.h>

#include <stdlib.h>
#include <stdio.h>

#define SUITE_NAME "bstring"
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

static void
test_reset(void)
{
    test_setup();
    test_teardown();
}

START_TEST(test_empty)
{
    struct bstring bstr;

    test_reset();

    bstring_init(&bstr);
    ck_assert_int_eq(bstring_empty(&bstr), 1);
    ck_assert_int_eq(bstring_copy(&bstr, "foo", 3), CC_OK);
    ck_assert_int_eq(bstring_empty(&bstr), 0);
    bstring_deinit(&bstr);
}
END_TEST

START_TEST(test_duplicate)
{
    struct bstring bstr1 = str2bstr("foo");
    struct bstring bstr2;

    test_reset();

    bstring_init(&bstr2);
    ck_assert_int_eq(bstring_duplicate(&bstr2, &bstr1), CC_OK);
    ck_assert_int_eq(bstr1.len, bstr2.len);
    ck_assert_int_eq(memcmp(bstr1.data, bstr2.data, bstr1.len), 0);

    bstring_deinit(&bstr2);
}
END_TEST

START_TEST(test_copy)
{
#define STR "foo"
    struct bstring bstr;

    test_reset();

    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, STR, sizeof(STR) - 1), CC_OK);
    ck_assert_int_eq(sizeof(STR) - 1, bstr.len);
    ck_assert_int_eq(memcmp(STR, bstr.data, bstr.len), 0);

    bstring_deinit(&bstr);
#undef STR
}
END_TEST

START_TEST(test_compare)
{
    struct bstring bstr1 = str2bstr("foo");
    struct bstring bstr2 = str2bstr("bar");
    struct bstring bstr3 = str2bstr("baz");

    test_reset();

    ck_assert_int_eq(bstring_compare(&bstr1, &bstr1), 0);
    ck_assert_int_gt(bstring_compare(&bstr1, &bstr2), 0);
    ck_assert_int_gt(bstring_compare(&bstr1, &bstr3), 0);
    ck_assert_int_lt(bstring_compare(&bstr2, &bstr1), 0);
    ck_assert_int_eq(bstring_compare(&bstr2, &bstr2), 0);
    ck_assert_int_lt(bstring_compare(&bstr2, &bstr3), 0);
    ck_assert_int_lt(bstring_compare(&bstr3, &bstr1), 0);
    ck_assert_int_gt(bstring_compare(&bstr3, &bstr2), 0);
    ck_assert_int_eq(bstring_compare(&bstr3, &bstr3), 0);
}
END_TEST

START_TEST(test_atou64)
{
    uint64_t val;
    struct bstring bstr;
    char max_uint64[CC_UINT64_MAXLEN];

    test_reset();

    ck_assert_int_eq(bstring_atou64(&val, &str2bstr("foo")), CC_ERROR);

    ck_assert_int_eq(bstring_atou64(&val, &str2bstr("-1")), CC_ERROR);

    ck_assert_int_eq(bstring_atou64(&val, &str2bstr("123")), CC_OK);
    ck_assert_uint_eq(val, 123);

    ck_assert_int_eq(bstring_atou64(&val, &str2bstr("123")), CC_OK);
    ck_assert_uint_eq(val, 123);

    sprintf(max_uint64, "%llu", UINT64_MAX);
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, max_uint64, strlen(max_uint64)), CC_OK);
    ck_assert_int_eq(bstring_atou64(&val, &bstr), CC_OK);
    ck_assert_uint_eq(val, UINT64_MAX);
    bstring_deinit(&bstr);

    sprintf(max_uint64, "%llu", UINT64_MAX);
    max_uint64[strlen(max_uint64) - 1]++;
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, max_uint64, strlen(max_uint64)), CC_OK);
    ck_assert_int_eq(bstring_atou64(&val, &bstr), CC_ERROR);
    bstring_deinit(&bstr);
}
END_TEST

/*
 * test suite
 */
static Suite *
bstring_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_bstring = tcase_create("cc_bstring test");
    suite_add_tcase(s, tc_bstring);

    tcase_add_test(tc_bstring, test_empty);
    tcase_add_test(tc_bstring, test_duplicate);
    tcase_add_test(tc_bstring, test_copy);
    tcase_add_test(tc_bstring, test_compare);
    tcase_add_test(tc_bstring, test_atou64);

    return s;
}
/**************
 * test cases *
 **************/

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = bstring_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
