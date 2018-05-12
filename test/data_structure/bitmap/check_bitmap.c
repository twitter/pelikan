#include <data_structure/bitmap/bitset.h>

#include <check.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "bitmap"
#define DEBUG_LOG  SUITE_NAME ".log"

#define BUF_SIZE 32

#define NCOL1 64
static uint16_t cols[4] = { 0, 7, 29, 42 };

static uint8_t buf[BUF_SIZE];
static struct bitset *bs = (struct bitset *)buf;


START_TEST(test_bitset_init)
{
    int i;
    uint8_t *d;

    for (i = 0; i < BUF_SIZE; i++) {
        buf[i] = 0xff;
    }

    bitset_init(bs, NCOL1);
    ck_assert_int_eq(bs->size, bit2long(NCOL1));
    ck_assert_int_eq(bs->count, 0);

    d = buf + offsetof(struct bitset, data);
    for (i = 0; i < bit2byte(NCOL1); i++, d++) {
        ck_assert_int_eq(*d, 0);
    }
}
END_TEST

START_TEST(test_bitset_getset)
{
    int i, j;

    bitset_init(bs, NCOL1);
    for (i = 0, j = 0; i < NCOL1; i++) {
        if (i == cols[j]) {
            bitset_set(bs, i, 1);
            j++;
            ck_assert_int_eq(bs->count, j);
        } else { /* only bits specified are set */
            ck_assert_int_eq(bitset_get(bs, i), 0);
        }
    }

    for (j=0; j < 4; j++) { /* set these bits back to 0 */
        ck_assert_int_eq(bitset_get(bs, cols[j]), 1);
        bitset_set(bs, cols[j], 0);
        ck_assert_int_eq(bitset_get(bs, cols[j]), 0);
        ck_assert_int_eq(bs->count, 3 - j);
    }
}
END_TEST

/*
 * test suite
 */
static Suite *
bitmap_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_bitset = tcase_create("bitset");
    suite_add_tcase(s, tc_bitset);

    tcase_add_test(tc_bitset, test_bitset_init);
    tcase_add_test(tc_bitset, test_bitset_getset);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = bitmap_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
