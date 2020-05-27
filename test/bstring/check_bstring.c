#include <cc_bstring.h>

#include <check.h>

#include <inttypes.h>
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

START_TEST(test_strcmp)
{
    ck_assert(str2cmp("an", 'a', 'n'));
    ck_assert(str3cmp("old", 'o', 'l', 'd'));
    ck_assert(str4cmp("farm", 'f', 'a', 'r', 'm'));
    ck_assert(str5cmp("EIEIO", 'E', 'I', 'E', 'I', 'O'));
    ck_assert(str6cmp("horses", 'h', 'o', 'r', 's', 'e', 's'));
    ck_assert(str7cmp("beavers", 'b', 'e', 'a', 'v', 'e', 'r', 's'));
    ck_assert(str8cmp("McDonald", 'M', 'c', 'D', 'o', 'n', 'a', 'l', 'd'));
    ck_assert(str9cmp("elephants", 'e', 'l', 'e', 'p', 'h', 'a', 'n', 't',
                's'));
    ck_assert(str10cmp("everywhere", 'e', 'v', 'e', 'r', 'y', 'w', 'h', 'e',
                'r', 'e'));
    ck_assert(str11cmp("polar bears", 'p', 'o', 'l', 'a', 'r', ' ', 'b', 'e',
                'a', 'r', 's'));
    ck_assert(str12cmp("snow leopard", 's', 'n', 'o', 'w', ' ', 'l', 'e', 'o',
                'p', 'a', 'r', 'd'));
    ck_assert(!str12cmp("pocket mouse", 's', 'n', 'o', 'w', ' ', 'l', 'e', 'o',
                'p', 'a', 'r', 'd'));
}
END_TEST


START_TEST(test_atoi64)
{
    int64_t val;
    struct bstring bstr;
    char int64[CC_INT64_MAXLEN];

    test_reset();

    ck_assert_int_eq(bstring_atoi64(&val, &str2bstr("foo")), CC_ERROR);

    ck_assert_int_eq(bstring_atoi64(&val, &str2bstr("123")), CC_OK);
    ck_assert_uint_eq(val, 123);
    ck_assert_int_eq(bstring_atoi64(&val, &str2bstr("-123")), CC_OK);
    ck_assert_uint_eq(val, -123);

    sprintf(int64, "%"PRIi64, INT64_MAX);
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, int64, strlen(int64)), CC_OK);
    ck_assert_int_eq(bstring_atoi64(&val, &bstr), CC_OK);
    ck_assert_int_eq(val, INT64_MAX);
    bstring_deinit(&bstr);

    sprintf(int64, "%"PRIi64, INT64_MIN);
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, int64, strlen(int64)), CC_OK);
    ck_assert_int_eq(bstring_atoi64(&val, &bstr), CC_OK);
    ck_assert_int_eq(val, INT64_MIN);
    bstring_deinit(&bstr);
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

    sprintf(max_uint64, "%"PRIu64, UINT64_MAX);
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, max_uint64, strlen(max_uint64)), CC_OK);
    ck_assert_int_eq(bstring_atou64(&val, &bstr), CC_OK);
    ck_assert_uint_eq(val, UINT64_MAX);
    bstring_deinit(&bstr);

    sprintf(max_uint64, "%"PRIu64, UINT64_MAX);
    max_uint64[strlen(max_uint64) - 1]++;
    bstring_init(&bstr);
    ck_assert_int_eq(bstring_copy(&bstr, max_uint64, strlen(max_uint64)), CC_OK);
    ck_assert_int_eq(bstring_atou64(&val, &bstr), CC_ERROR);
    bstring_deinit(&bstr);
}
END_TEST

START_TEST(test_bstring_alloc_and_free)
{
#define BSTRING_SIZE 9000

    struct bstring *bs;

    bs = bstring_alloc(BSTRING_SIZE);
    ck_assert_uint_eq(bs->len, BSTRING_SIZE);
    for (int i = 0; i < BSTRING_SIZE; i++) {
        bs->data[i] = 'a';
    }

    /* great! we didn't segfault! */
    bstring_free(&bs);
    ck_assert_ptr_null(bs);

#undef BSTRING_SIZE
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
    tcase_add_test(tc_bstring, test_strcmp);
    tcase_add_test(tc_bstring, test_atoi64);
    tcase_add_test(tc_bstring, test_atou64);
    tcase_add_test(tc_bstring, test_bstring_alloc_and_free);

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
