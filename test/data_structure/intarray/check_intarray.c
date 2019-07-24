#include <data_structure/intarray/intarray.h>

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

START_TEST(test_intarray_create)
{
    ck_assert_int_eq(intarray_init(buf, 1), INTARRAY_OK);
    ck_assert_int_eq(intarray_init(buf, 2), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 0);
    ck_assert_int_eq(intarray_init(buf, 4), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 0);
    ck_assert_int_eq(intarray_init(buf, 8), INTARRAY_OK);
    ck_assert_int_eq(intarray_init(buf, 16), INTARRAY_EINVALID);
    ck_assert_int_eq(intarray_init(buf, 3), INTARRAY_EINVALID);
}
END_TEST


START_TEST(test_intarray_insert_seek)
{
    uint32_t idx;
    uint64_t val;

    ck_assert_int_eq(intarray_init(buf, 1), INTARRAY_OK);
    ck_assert_int_eq(intarray_insert(buf, 1), INTARRAY_OK);
    ck_assert_int_eq(intarray_insert(buf, 3), INTARRAY_OK);
    ck_assert_int_eq(intarray_insert(buf, 5), INTARRAY_OK);
    ck_assert_int_eq(intarray_insert(buf, 12345), INTARRAY_EINVALID);
    ck_assert_int_eq(intarray_nentry(buf), 3);
    ck_assert_int_eq(intarray_value(&val, buf, 1), INTARRAY_OK);
    ck_assert_int_eq(val, 3);
    ck_assert_int_eq(intarray_index(&idx, buf, 3), INTARRAY_OK);
    ck_assert_int_eq(idx, 1);
    ck_assert_int_eq(intarray_index(&idx, buf, 2), INTARRAY_ENOTFOUND);

    ck_assert_int_eq(intarray_init(buf, 8), INTARRAY_OK);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        ck_assert_int_eq(intarray_insert(buf, 1000 + i * 2), INTARRAY_OK);
    }
    ck_assert_int_eq(intarray_nentry(buf), 50);
    ck_assert_int_eq(intarray_value(&val, buf, 10), INTARRAY_OK);
    ck_assert_int_eq(val, 1020);
    ck_assert_int_eq(intarray_index(&idx, buf, 1020), INTARRAY_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(intarray_index(&idx, buf, 1000), INTARRAY_OK);
    ck_assert_int_eq(idx, 0);
    ck_assert_int_eq(intarray_index(&idx, buf, 1098), INTARRAY_OK);
    ck_assert_int_eq(idx, 49);
    ck_assert_int_eq(intarray_index(&idx, buf, 1), INTARRAY_ENOTFOUND);
    ck_assert_int_eq(intarray_index(&idx, buf, 2000), INTARRAY_ENOTFOUND);
}
END_TEST

START_TEST(test_intarray_remove)
{
    uint32_t idx;

    intarray_init(buf, 1);
    intarray_insert(buf, 1);
    intarray_insert(buf, 3);
    intarray_insert(buf, 5);
    ck_assert_int_eq(intarray_remove(buf, 12345), INTARRAY_EINVALID);

    intarray_init(buf, 8);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        intarray_insert(buf, 1000 + i * 2);
    }
    ck_assert_int_eq(intarray_nentry(buf), 50);
    ck_assert_int_eq(intarray_remove(buf, 1020), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 49);
    ck_assert_int_eq(intarray_index(&idx, buf, 1020), INTARRAY_ENOTFOUND);
    ck_assert_int_eq(intarray_index(&idx, buf, 1022), INTARRAY_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(intarray_remove(buf, 1000), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 48);
    ck_assert_int_eq(intarray_index(&idx, buf, 1000), INTARRAY_ENOTFOUND);
    ck_assert_int_eq(intarray_remove(buf, 1098), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 47);
    ck_assert_int_eq(intarray_index(&idx, buf, 1098), INTARRAY_ENOTFOUND);
}
END_TEST

START_TEST(test_intarray_truncate)
{
    uint32_t idx;

    intarray_init(buf, 8);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        intarray_insert(buf, 1000 + i * 2);
    }
    ck_assert_int_eq(intarray_nentry(buf), 50);
    ck_assert_int_eq(intarray_truncate(buf, -10), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 40);
    ck_assert_int_eq(intarray_index(&idx, buf, 1080), INTARRAY_ENOTFOUND);
    ck_assert_int_eq(intarray_index(&idx, buf, 1078), INTARRAY_OK);
    ck_assert_int_eq(intarray_truncate(buf, 10), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 30);
    ck_assert_int_eq(intarray_index(&idx, buf, 1018), INTARRAY_ENOTFOUND);
    ck_assert_int_eq(intarray_index(&idx, buf, 1020), INTARRAY_OK);
    ck_assert_int_eq(intarray_truncate(buf, 31), INTARRAY_OK);
    ck_assert_int_eq(intarray_nentry(buf), 0);
}
END_TEST


/*
 * test suite
 */
static Suite *
intarray_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_intarray = tcase_create("intarray");
    suite_add_tcase(s, tc_intarray);

    tcase_add_test(tc_intarray, test_intarray_create);
    tcase_add_test(tc_intarray, test_intarray_insert_seek);
    tcase_add_test(tc_intarray, test_intarray_remove);
    tcase_add_test(tc_intarray, test_intarray_truncate);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = intarray_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
