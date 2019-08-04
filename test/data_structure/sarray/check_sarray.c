#include <data_structure/sarray/sarray.h>

#include <cc_mm.h>
#include <stdio.h>
#include <stdlib.h>

#include <check.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "intarray"
#define DEBUG_LOG  SUITE_NAME ".log"

#define BUF_SIZE 8200
#define NENTRY   1024

static unsigned char buf[BUF_SIZE];


/*
 * intarray tests
 */

START_TEST(test_sarray_create)
{
    ck_assert_int_eq(sarray_init(buf, 1), SARRAY_OK);
    ck_assert_int_eq(sarray_init(buf, 2), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 0);
    ck_assert_int_eq(sarray_init(buf, 4), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 0);
    ck_assert_int_eq(sarray_init(buf, 8), SARRAY_OK);
    ck_assert_int_eq(sarray_init(buf, 16), SARRAY_EINVALID);
    ck_assert_int_eq(sarray_init(buf, 3), SARRAY_EINVALID);
}
END_TEST


START_TEST(test_sarray_insert_seek)
{
    uint32_t idx;
    uint64_t val;

    ck_assert_int_eq(sarray_init(buf, 1), SARRAY_OK);
    ck_assert_int_eq(sarray_insert(buf, 3), SARRAY_OK);  /* [3] */
    ck_assert_int_eq(sarray_insert(buf, 1), SARRAY_OK);  /* [1, 3] */
    ck_assert_int_eq(sarray_insert(buf, 5), SARRAY_OK);  /* [1, 3, 5] */
    ck_assert_int_eq(sarray_insert(buf, 12345), SARRAY_EINVALID);
    ck_assert_int_eq(sarray_nentry(buf), 3);
    ck_assert_int_eq(sarray_value(&val, buf, 1), SARRAY_OK);
    ck_assert_int_eq(val, 3);
    ck_assert_int_eq(sarray_index(&idx, buf, 3), SARRAY_OK);
    ck_assert_int_eq(idx, 1);
    ck_assert_int_eq(sarray_index(&idx, buf, 2), SARRAY_ENOTFOUND);

    ck_assert_int_eq(sarray_init(buf, 8), SARRAY_OK);
    for (int i = 49; i >= 0; --i) {
        ck_assert_int_eq(sarray_insert(buf, 1000 + i * 2), SARRAY_OK);
    }
    ck_assert_int_eq(sarray_nentry(buf), 50);
    ck_assert_int_eq(sarray_value(&val, buf, 0), SARRAY_OK);
    ck_assert_int_eq(val, 1000);
    ck_assert_int_eq(sarray_value(&val, buf, 10), SARRAY_OK);
    ck_assert_int_eq(val, 1020);
    ck_assert_int_eq(sarray_index(&idx, buf, 1020), SARRAY_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(sarray_index(&idx, buf, 1000), SARRAY_OK);
    ck_assert_int_eq(idx, 0);
    ck_assert_int_eq(sarray_index(&idx, buf, 1098), SARRAY_OK);
    ck_assert_int_eq(idx, 49);
    ck_assert_int_eq(sarray_index(&idx, buf, 1), SARRAY_ENOTFOUND);
    ck_assert_int_eq(sarray_index(&idx, buf, 2000), SARRAY_ENOTFOUND);
}
END_TEST

START_TEST(test_sarray_remove)
{
    uint32_t idx;

    sarray_init(buf, 1);
    sarray_insert(buf, 1);
    sarray_insert(buf, 3);
    sarray_insert(buf, 5);
    ck_assert_int_eq(sarray_remove(buf, 12345), SARRAY_EINVALID);

    sarray_init(buf, 8);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        sarray_insert(buf, 1000 + i * 2);
    }
    ck_assert_int_eq(sarray_nentry(buf), 50);
    ck_assert_int_eq(sarray_remove(buf, 1020), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 49);
    ck_assert_int_eq(sarray_index(&idx, buf, 1020), SARRAY_ENOTFOUND);
    ck_assert_int_eq(sarray_index(&idx, buf, 1022), SARRAY_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(sarray_remove(buf, 1000), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 48);
    ck_assert_int_eq(sarray_index(&idx, buf, 1000), SARRAY_ENOTFOUND);
    ck_assert_int_eq(sarray_remove(buf, 1098), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 47);
    ck_assert_int_eq(sarray_index(&idx, buf, 1098), SARRAY_ENOTFOUND);
}
END_TEST

START_TEST(test_sarray_truncate)
{
    uint32_t idx;

    sarray_init(buf, 8);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        sarray_insert(buf, 1000 + i * 2);
    }
    ck_assert_int_eq(sarray_nentry(buf), 50);
    ck_assert_int_eq(sarray_truncate(buf, -10), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 40);
    ck_assert_int_eq(sarray_index(&idx, buf, 1080), SARRAY_ENOTFOUND);
    ck_assert_int_eq(sarray_index(&idx, buf, 1078), SARRAY_OK);
    ck_assert_int_eq(sarray_truncate(buf, 10), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 30);
    ck_assert_int_eq(sarray_index(&idx, buf, 1018), SARRAY_ENOTFOUND);
    ck_assert_int_eq(sarray_index(&idx, buf, 1020), SARRAY_OK);
    ck_assert_int_eq(sarray_truncate(buf, 31), SARRAY_OK);
    ck_assert_int_eq(sarray_nentry(buf), 0);
}
END_TEST


/*
 * test suite
 */
static Suite *
sarray_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_intarray = tcase_create("intarray");
    suite_add_tcase(s, tc_intarray);

    tcase_add_test(tc_intarray, test_sarray_create);
    tcase_add_test(tc_intarray, test_sarray_insert_seek);
    tcase_add_test(tc_intarray, test_sarray_remove);
    tcase_add_test(tc_intarray, test_sarray_truncate);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = sarray_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
