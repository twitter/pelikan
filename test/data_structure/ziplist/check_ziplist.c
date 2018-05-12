#include <data_structure/ziplist/ziplist.h>

#include <cc_mm.h>

#include <check.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "ziplist"
#define DEBUG_LOG  SUITE_NAME ".log"

struct ze_example {
    char *      encoded;
    uint8_t     nbyte;
    struct blob decoded;
};

static struct ze_example ze_examples[] = {
    /* ZE_U8 */
    {"\x00\x02", 2, {.type=BLOB_TYPE_INT, .vint=0}},
    {"\xfa\x02", 2, {.type=BLOB_TYPE_INT, .vint=ZE_U8_MAX}},
    /* ZE_U16 */
    {"\xfb\xfb\x00\x04", 4, {.type=BLOB_TYPE_INT, .vint=ZE_U8_MAX + 1}},
    {"\xfb\xff\xff\x04", 4, {.type=BLOB_TYPE_INT, .vint=ZE_U16_MAX}},
    /* ZE_U24 */
    {"\xfc\x00\x00\x01\x05", 5, {.type=BLOB_TYPE_INT, .vint=ZE_U16_MAX + 1}},
    {"\xfc\xff\xff\xff\x05", 5, {.type=BLOB_TYPE_INT, .vint=ZE_U24_MAX}},
    /* ZE_U56 */
    {"\xfd\x00\x00\x00\x01\x00\x00\x00\x09", 9,
        {.type=BLOB_TYPE_INT, .vint=ZE_U24_MAX + 1}},
    {"\xfd\xff\xff\xff\xff\xff\xff\xff\x09", 9,
        {.type=BLOB_TYPE_INT, .vint=ZE_U56_MAX}},
    /* ZE_U64 */
    {"\xfe\x00\x00\x00\x00\x00\x00\x00\x01\x0a", 10,
        {.type=BLOB_TYPE_INT, .vint=ZE_U56_MAX + 1}},
    {"\xfe\xff\xff\xff\xff\xff\xff\xff\xff\x0a", 10,
        {.type=BLOB_TYPE_INT, .vint=ZE_U64_MAX}},
    /* ZE_STR */
    {"\xff\x0b\x48\x65\x6c\x6c\x6f\x20\x57\x6f\x72\x6c\x64\x0e", 14,
        {.type=BLOB_TYPE_STR, .vstr={11, "Hello World"}}},
};

#define BUF_SIZE 10240
#define NENTRY   1024

static int n_ze = sizeof(ze_examples) / sizeof(struct ze_example);
static struct blob val;
static char ref[BUF_SIZE];
static char buf[BUF_SIZE];
static zipentry_p ze_index[NENTRY];


/*
 * zipentry tests
 */

START_TEST(test_zipentry_get)
{
    int i;
    uint8_t sz;

    for (i = 0; i < n_ze; ++i) {
        ck_assert_int_eq(zipentry_get(&val, (zipentry_p)ze_examples[i].encoded),
                ZIPLIST_OK);
        ck_assert_int_eq(zipentry_size(&sz, &val), ZIPLIST_OK);
        ck_assert_int_eq(sz, ze_examples[i].nbyte);
        ck_assert_int_eq(val.type, ze_examples[i].decoded.type);
        if (val.type == BLOB_TYPE_INT) {
            ck_assert_int_eq(val.vint, ze_examples[i].decoded.vint);
        } else {
            ck_assert_int_eq(val.vstr.len, ze_examples[i].decoded.vstr.len);
            ck_assert(memcmp(val.vstr.data, ze_examples[i].decoded.vstr.data,
                        val.vstr.len) == 0);
        }
    }

    ck_assert_int_eq(zipentry_get(&val, NULL), ZIPLIST_ERROR);
}

END_TEST
START_TEST(test_zipentry_set)
{
    int i;

    for (i = 0; i < n_ze; ++i) {
        ck_assert_int_eq(zipentry_set((zipentry_p)buf, &ze_examples[i].decoded),
                ZIPLIST_OK);
        ck_assert_int_eq(memcmp(buf, ze_examples[i].encoded,
                    ze_examples[i].nbyte), 0);
        ck_assert_int_eq(zipentry_compare((zipentry_p)buf,
                    &ze_examples[i].decoded), 0);
    }

    ck_assert_int_eq(zipentry_set(NULL, &val), ZIPLIST_ERROR);
    ck_assert_int_eq(zipentry_set((zipentry_p)buf, NULL), ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert_int_eq(zipentry_set((zipentry_p)buf, &val), ZIPLIST_EINVALID);
}
END_TEST

START_TEST(test_zipentry_compare)
{
    int i;

    for (i = 0; i < n_ze - 1; ++i) {
        ck_assert_int_eq(zipentry_compare((zipentry_p)ze_examples[i].encoded,
                    &ze_examples[i + 1].decoded), -1);
    }
    for (; i > 1; --i) {
        ck_assert_int_eq(zipentry_compare((zipentry_p)ze_examples[i].encoded,
                    &ze_examples[i - 1].decoded), 1);
    }
}
END_TEST

/*
 * ziplist tests
 */

START_TEST(test_ziplist_seek)
{
    int i;
    zipentry_p ze;

    /* prev & next */
    for (i = 0; i < n_ze - 1; ++i) {
        ck_assert(ziplist_next(&ze, (ziplist_p)ref, ze_index[i]) == ZIPLIST_OK);
        ck_assert(ze == ze_index[i + 1]);
    }

    ck_assert(ziplist_next(&ze, (ziplist_p)ref, ze_index[i]) == ZIPLIST_EOOB);
    for (; i > 1; --i) {
        ck_assert(ziplist_prev(&ze, (ziplist_p)ref, ze_index[i]) == ZIPLIST_OK);
        ck_assert(ze == ze_index[i - 1]);
    }
    ck_assert(ziplist_prev(&ze, (ziplist_p)ref, ze_index[0]) == ZIPLIST_EOOB);

    /* locate */
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_locate(&ze, (ziplist_p)ref, i) == ZIPLIST_OK);
        ck_assert(ze == ze_index[i]);
    }

    ck_assert(ziplist_locate(&ze, (ziplist_p)ref, n_ze) == ZIPLIST_EOOB);
    ck_assert(ziplist_locate(NULL, (ziplist_p)ref, 0) == ZIPLIST_ERROR);
    ck_assert(ziplist_locate(&ze, NULL, 0) == ZIPLIST_ERROR);

    /* find */
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_find(&ze, (ziplist_p)ref, &ze_examples[i].decoded) ==
                ZIPLIST_OK);
        ck_assert(ze == ze_index[i]);
    }
    val = (struct blob){.type=BLOB_TYPE_INT, .vint=42};
    ck_assert(ziplist_find(&ze, (ziplist_p)ref, &val) == ZIPLIST_OK);
    ck_assert(ze == NULL);
    val = (struct blob){.type=BLOB_TYPE_STR, .vstr=(struct bstring){2, "pi"}};
    ck_assert(ziplist_find(&ze, (ziplist_p)ref, &val) == ZIPLIST_OK);
    ck_assert(ze == NULL);

    ck_assert(ziplist_find(NULL, (ziplist_p)ref, &val) == ZIPLIST_ERROR);
    ck_assert(ziplist_find(&ze, NULL, &val) == ZIPLIST_ERROR);
    ck_assert(ziplist_find(&ze, (ziplist_p)ref, NULL) == ZIPLIST_ERROR);

}
END_TEST

START_TEST(test_ziplist_modify)
{
    int i;
    int64_t idx[NENTRY];
    uint32_t offset[NENTRY];

    /* reset */
    for (i = 0; i < ZIPLIST_HEADER_SIZE; ++i) {
        buf[i] = 0xff;
    }
    ck_assert(ziplist_reset((ziplist_p)buf) == ZIPLIST_OK);
    ck_assert(ziplist_reset((ziplist_p)buf) == ZIPLIST_OK);

    ck_assert(ziplist_reset(NULL) == ZIPLIST_ERROR);

    /* push */
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_push((ziplist_p)buf, &ze_examples[i].decoded) ==
                ZIPLIST_OK);
    }
    ck_assert(memcmp(ref, buf, ziplist_len((ziplist_p)ref)) == 0);

    ck_assert(ziplist_push(NULL, &val) == ZIPLIST_ERROR);
    ck_assert(ziplist_push((ziplist_p)buf, NULL) == ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert(ziplist_push((ziplist_p)buf, &val) == ZIPLIST_EINVALID);

    /* insert */
    /* going from ends to middle,
     * insert position: 0, 1, 1, 2, 2, ...
     * entry index: 0, n_ze - 1, 1, n_ze - 2, ...
     */
    for (i = 0; i < n_ze; ++i) {
        idx[i] = (i + 1) / 2;
        if (i % 2 == 0) {
            offset[i] = i / 2;
        } else {
            offset[i] = n_ze - 1 - i / 2;
        }
    }

    ziplist_reset((ziplist_p)buf);
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_insert((ziplist_p)buf,
                &ze_examples[offset[i]].decoded, idx[i]) == ZIPLIST_OK);
    }
    ck_assert(memcmp(ref, buf, ziplist_len((ziplist_p)ref)) == 0);

    ck_assert(ziplist_insert(NULL, &val, 0) == ZIPLIST_ERROR);
    ck_assert(ziplist_insert((ziplist_p)buf, NULL, 0) == ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert(ziplist_insert((ziplist_p)buf, &val, 0) == ZIPLIST_EINVALID);
    val.vstr.len = 1;
    ck_assert(ziplist_insert((ziplist_p)buf, &val, n_ze + 1) == ZIPLIST_EOOB);
}
END_TEST


/*
 * test suite
 */
static Suite *
zipmap_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);
    int i;
    uint32_t sz;

    TCase *tc_zipentry = tcase_create("zipentry");
    suite_add_tcase(s, tc_zipentry);

    tcase_add_test(tc_zipentry, test_zipentry_get);
    tcase_add_test(tc_zipentry, test_zipentry_set);
    tcase_add_test(tc_zipentry, test_zipentry_compare);

    /* create a reference ziplist */
    for (i = 0, sz = ZIPLIST_HEADER_SIZE; i < n_ze; ++i) {
        ze_index[i] = (zipentry_p)(ref+ sz);
        cc_memcpy(ref + sz, ze_examples[i].encoded, ze_examples[i].nbyte);
        sz += ze_examples[i].nbyte;
    }

    *((uint32_t *)ref) = n_ze;
    *((uint32_t *)ref + 1) = sz - 1;

    TCase *tc_ziplist = tcase_create("ziplist");
    suite_add_tcase(s, tc_ziplist);
    tcase_add_test(tc_ziplist, test_ziplist_seek);
    tcase_add_test(tc_ziplist, test_ziplist_modify);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = zipmap_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
