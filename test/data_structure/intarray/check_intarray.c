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
    ck_assert(intarray_init(buf, 2) == INTARRAY_OK);
    ck_assert(intarray_init(buf, 4) == INTARRAY_OK);
    ck_assert(intarray_init(buf, 8) == INTARRAY_OK);
    ck_assert(intarray_init(buf, 16) == INTARRAY_EINVALID);
    ck_assert(intarray_init(buf, 3) == INTARRAY_EINVALID);
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
