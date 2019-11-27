#include <data_structure/smap/smap.h>

#include <cc_bstring.h>
#include <cc_mm.h>
#include <stdio.h>
#include <stdlib.h>

#include <check.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "smap"
#define DEBUG_LOG  SUITE_NAME ".log"

#define NENTRY   1024
#define VLEN 16
#define VALUE "0123456789abcdef"

#define BUF_SIZE (NENTRY * (8 + VLEN) + SMAP_HEADER_SIZE)

static char buf[BUF_SIZE];
static struct bstring val = str2bstr(VALUE);


/*
 * intarray tests
 */

START_TEST(test_smap_create)
{
    ck_assert_int_eq(smap_init(buf, 1, VLEN), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 17);
    ck_assert_int_eq(smap_nentry(buf), 0);
    ck_assert_int_eq(smap_init(buf, 2, VLEN), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 18);
    ck_assert_int_eq(smap_init(buf, 4, VLEN), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 20);
    ck_assert_int_eq(smap_init(buf, 8, VLEN), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 24);
    ck_assert_int_eq(smap_init(buf, 16, VLEN), SMAP_EINVALID);
    ck_assert_int_eq(smap_init(buf, 3, VLEN), SMAP_EINVALID);
    ck_assert_int_eq(smap_init(buf, 8, 7), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 16);
    ck_assert_int_eq(smap_init(buf, 4, 2), SMAP_OK);
    ck_assert_int_eq(smap_esize(buf), 8);
}
END_TEST


START_TEST(test_smap_insert_seek)
{
    uint32_t idx;
    uint64_t key;
    struct bstring val_read;

    ck_assert_int_eq(smap_init(buf, 1, VLEN), SMAP_OK);
    ck_assert_int_eq(smap_insert(buf, 3, &val), SMAP_OK);  /* [(3, val)] */
    ck_assert_int_eq(smap_insert(buf, 1, &val), SMAP_OK);  /* [(1, val), (3, val)] */
    ck_assert_int_eq(smap_insert(buf, 5, &val), SMAP_OK);  /* [(1, val), (3, val)], (5, val)] */
    ck_assert_int_eq(smap_insert(buf, 12345, &val), SMAP_EINVALID);
    ck_assert_int_eq(smap_nentry(buf), 3);
    ck_assert_int_eq(smap_keyval(&key, &val_read, buf, 1), SMAP_OK);
    ck_assert_int_eq(key, 3);
    ck_assert_int_eq(bstring_compare(&val, &val_read), 0);
    ck_assert_int_eq(smap_index(&idx, buf, 3), SMAP_OK);
    ck_assert_int_eq(idx, 1);
    ck_assert_int_eq(smap_index(&idx, buf, 2), SMAP_ENOTFOUND);

    ck_assert_int_eq(smap_init(buf, 8, VLEN), SMAP_OK);
    for (int i = 49; i >= 0; --i) {
        ck_assert_int_eq(smap_insert(buf, 1000 + i * 2, &val), SMAP_OK);
    }
    ck_assert_int_eq(smap_nentry(buf), 50);
    ck_assert_int_eq(smap_keyval(&key, &val_read, buf, 0), SMAP_OK);
    ck_assert_int_eq(key, 1000);
    ck_assert_int_eq(smap_keyval(&key, &val_read, buf, 10), SMAP_OK);
    ck_assert_int_eq(key, 1020);
    ck_assert_int_eq(smap_index(&idx, buf, 1020), SMAP_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(smap_index(&idx, buf, 1000), SMAP_OK);
    ck_assert_int_eq(idx, 0);
    ck_assert_int_eq(smap_index(&idx, buf, 1098), SMAP_OK);
    ck_assert_int_eq(idx, 49);
    ck_assert_int_eq(smap_index(&idx, buf, 1), SMAP_ENOTFOUND);
    ck_assert_int_eq(smap_index(&idx, buf, 2000), SMAP_ENOTFOUND);
}
END_TEST

START_TEST(test_smap_remove)
{
    uint32_t idx;

    smap_init(buf, 1, VLEN);
    smap_insert(buf, 1, &val);
    smap_insert(buf, 3, &val);
    smap_insert(buf, 5, &val);
    ck_assert_int_eq(smap_remove(buf, 12345), SMAP_EINVALID);

    smap_init(buf, 8, VLEN);
    for (int i = 0; i < 50; ++i) { /* key: 0, 2, 4, ..., 98 */
        smap_insert(buf, 1000 + i * 2, &val);
    }
    ck_assert_int_eq(smap_nentry(buf), 50);
    ck_assert_int_eq(smap_remove(buf, 1020), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 49);
    ck_assert_int_eq(smap_index(&idx, buf, 1020), SMAP_ENOTFOUND);
    ck_assert_int_eq(smap_index(&idx, buf, 1022), SMAP_OK);
    ck_assert_int_eq(idx, 10);
    ck_assert_int_eq(smap_remove(buf, 1000), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 48);
    ck_assert_int_eq(smap_index(&idx, buf, 1000), SMAP_ENOTFOUND);
    ck_assert_int_eq(smap_remove(buf, 1098), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 47);
    ck_assert_int_eq(smap_index(&idx, buf, 1098), SMAP_ENOTFOUND);
}
END_TEST

START_TEST(test_smap_truncate)
{
    uint32_t idx;

    smap_init(buf, 8, VLEN);
    for (int i = 0; i < 50; ++i) { /* 0, 2, 4, ..., 98 */
        smap_insert(buf, 1000 + i * 2, &val);
    }
    ck_assert_int_eq(smap_nentry(buf), 50);
    ck_assert_int_eq(smap_truncate(buf, -10), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 40);
    ck_assert_int_eq(smap_index(&idx, buf, 1080), SMAP_ENOTFOUND);
    ck_assert_int_eq(smap_index(&idx, buf, 1078), SMAP_OK);
    ck_assert_int_eq(smap_truncate(buf, 10), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 30);
    ck_assert_int_eq(smap_index(&idx, buf, 1018), SMAP_ENOTFOUND);
    ck_assert_int_eq(smap_index(&idx, buf, 1020), SMAP_OK);
    ck_assert_int_eq(smap_truncate(buf, 31), SMAP_OK);
    ck_assert_int_eq(smap_nentry(buf), 0);
}
END_TEST


/*
 * test suite
 */
static Suite *
smap_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    TCase *tc_smap = tcase_create("smap");
    suite_add_tcase(s, tc_smap);

    tcase_add_test(tc_smap, test_smap_create);
    tcase_add_test(tc_smap, test_smap_insert_seek);
    tcase_add_test(tc_smap, test_smap_remove);
    tcase_add_test(tc_smap, test_smap_truncate);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = smap_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
