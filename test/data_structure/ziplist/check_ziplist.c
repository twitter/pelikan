#include <data_structure/ziplist/ziplist.h>

#include <cc_mm.h>
#include <stdio.h>

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
    {"\x00\x02", 2, {.type = BLOB_TYPE_INT, .vint = 0}},
    {"\xfa\x02", 2, {.type = BLOB_TYPE_INT, .vint = ZE_U8_MAX}},
    /* ZE_U16 */
    {"\xfb\xfb\x00\x04", 4, {.type = BLOB_TYPE_INT, .vint = ZE_U8_MAX + 1}},
    {"\xfb\xff\xff\x04", 4, {.type = BLOB_TYPE_INT, .vint = ZE_U16_MAX}},
    /* ZE_U24 */
    {"\xfc\x00\x00\x01\x05", 5, {.type = BLOB_TYPE_INT, .vint = ZE_U16_MAX + 1}},
    {"\xfc\xff\xff\xff\x05", 5, {.type = BLOB_TYPE_INT, .vint = ZE_U24_MAX}},
    /* ZE_U56 */
    {"\xfd\x00\x00\x00\x01\x00\x00\x00\x09", 9,
        {.type = BLOB_TYPE_INT, .vint = ZE_U24_MAX + 1}},
    {"\xfd\xff\xff\xff\xff\xff\xff\xff\x09", 9,
        {.type = BLOB_TYPE_INT, .vint = ZE_U56_MAX}},
    /* ZE_U64 */
    {"\xfe\x00\x00\x00\x00\x00\x00\x00\x01\x0a", 10,
        {.type = BLOB_TYPE_INT, .vint = ZE_U56_MAX + 1}},
    {"\xfe\xff\xff\xff\xff\xff\xff\xff\xff\x0a", 10,
        {.type = BLOB_TYPE_INT, .vint = ZE_U64_MAX}},
    /* ZE_STR */
    {"\xff\x0b\x48\x65\x6c\x6c\x6f\x20\x57\x6f\x72\x6c\x64\x0e", 14,
        {.type = BLOB_TYPE_STR, .vstr = {11, "Hello World"}}},
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
        ck_assert(blob_compare(&val, &ze_examples[i].decoded) == 0);
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
    for(; i < n_ze; ++i) {
        ck_assert_int_eq(zipentry_compare((zipentry_p)ze_examples[i].encoded,
                    &ze_examples[i].decoded), 0);
    }
}
END_TEST

/*
 * ziplist tests
 */

START_TEST(test_ziplist_seeklocation)
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
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_locate(&ze, (ziplist_p)ref, -1 - i) == ZIPLIST_OK);
        ck_assert(ze == ze_index[n_ze - 1 - i]);
    }

    ck_assert(ziplist_locate(&ze, (ziplist_p)ref, n_ze) == ZIPLIST_EOOB);
    ck_assert(ziplist_locate(NULL, (ziplist_p)ref, 0) == ZIPLIST_ERROR);
    ck_assert(ziplist_locate(&ze, NULL, 0) == ZIPLIST_ERROR);
}
END_TEST

START_TEST(test_ziplist_seekvalue)
{
    int i;
    int64_t idx;
    zipentry_p ze;

    /* find */
    for (i = 0; i < n_ze; ++i) {
        ck_assert(ziplist_find(&ze, &idx, (ziplist_p)ref,
                &ze_examples[i].decoded) == ZIPLIST_OK);
        ck_assert(idx == i);
        ck_assert(ze == ze_index[i]);
    }
    ck_assert(ziplist_find(&ze, NULL, (ziplist_p)ref, &ze_examples[0].decoded)
            == ZIPLIST_OK);
    ck_assert(ziplist_find(NULL, &idx, (ziplist_p)ref, &ze_examples[0].decoded)
            == ZIPLIST_OK);
    val = (struct blob){.type=BLOB_TYPE_INT, .vint=42};
    ck_assert(ziplist_find(&ze, &idx, (ziplist_p)ref, &val) ==
            ZIPLIST_ENOTFOUND);
    ck_assert(ze == NULL);
    ck_assert(idx == -1);
    val = (struct blob){.type=BLOB_TYPE_STR, .vstr=(struct bstring){2, "pi"}};
    ck_assert(ziplist_find(&ze, &idx, (ziplist_p)ref, &val) ==
            ZIPLIST_ENOTFOUND);
    ck_assert(ze == NULL);
    ck_assert(idx == -1);

    ck_assert(ziplist_find(NULL, NULL, (ziplist_p)ref, &val) == ZIPLIST_ENOTFOUND);
    ck_assert(ziplist_find(&ze, &idx, NULL, &val) == ZIPLIST_ERROR);
    ck_assert(ziplist_find(&ze, &idx, (ziplist_p)ref, NULL) == ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert(ziplist_find(&ze, &idx, (ziplist_p)ref, &val) ==
            ZIPLIST_EINVALID);

}
END_TEST

START_TEST(test_ziplist_resetpushpop)
{
    int i;

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
    ck_assert(memcmp(ref, buf, ziplist_size((ziplist_p)ref)) == 0);

    ck_assert(ziplist_push(NULL, &val) == ZIPLIST_ERROR);
    ck_assert(ziplist_push((ziplist_p)buf, NULL) == ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert(ziplist_push((ziplist_p)buf, &val) == ZIPLIST_EINVALID);

    /* pop */
    for (i = n_ze - 1; i >= 0; --i) {
        ck_assert(ziplist_pop(&val, (ziplist_p)buf) == ZIPLIST_OK);
        ck_assert_int_eq(val.type, ze_examples[i].decoded.type);
        ck_assert(blob_compare(&val, &ze_examples[i].decoded) == 0);
    }

    ziplist_push((ziplist_p)buf, &ze_examples[0].decoded);
    ck_assert(ziplist_pop(NULL, (ziplist_p)buf) == ZIPLIST_OK);

    ck_assert(ziplist_pop(&val, NULL) == ZIPLIST_ERROR);
    ck_assert(ziplist_pop(&val, (ziplist_p)buf) == ZIPLIST_EOOB);
}
END_TEST

START_TEST(test_ziplist_insertremove)
{
    int i;
    int64_t idx[NENTRY], d;
    uint32_t offset[NENTRY];

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
    ck_assert(memcmp(ref, buf, ziplist_size((ziplist_p)ref)) == 0);
    /* using negative index, but we need to fill out the ends first */
    ziplist_reset((ziplist_p)buf);
    ziplist_insert((ziplist_p)buf, &ze_examples[0].decoded, 0);
    ziplist_insert((ziplist_p)buf, &ze_examples[n_ze - 1].decoded, 1);
    for (i = 2; i < n_ze; ++i) {
        ck_assert(ziplist_insert((ziplist_p)buf,
                &ze_examples[offset[i]].decoded, -idx[i - 1]) == ZIPLIST_OK);
    }
    ck_assert(memcmp(ref, buf, ziplist_size((ziplist_p)ref)) == 0);

    ck_assert(ziplist_insert(NULL, &val, 0) == ZIPLIST_ERROR);
    ck_assert(ziplist_insert((ziplist_p)buf, NULL, 0) == ZIPLIST_ERROR);
    val.type = BLOB_TYPE_STR;
    val.vstr.len = ZE_STR_MAXLEN + 1;
    ck_assert(ziplist_insert((ziplist_p)buf, &val, 0) == ZIPLIST_EINVALID);
    val.vstr.len = 1;
    ck_assert(ziplist_insert((ziplist_p)buf, &val, n_ze + 1) == ZIPLIST_EOOB);

    /* remove: reverse insertion order, from middle to ends  */
    for (i = n_ze - 1; i >= 0; --i) {
        ziplist_find(NULL, &d, (ziplist_p)buf, &ze_examples[offset[i]].decoded);
        ck_assert_int_eq(d, idx[i]);
        ck_assert(ziplist_remove((ziplist_p)buf, d, 1) == ZIPLIST_OK);
        ziplist_find(NULL, &d, (ziplist_p)buf, &ze_examples[offset[i]].decoded);
        ck_assert_int_eq(d, -1);
    }
    ck_assert_int_eq(ziplist_nentry((ziplist_p)buf), 0);

    ziplist_insert((ziplist_p)buf, &ze_examples[0].decoded, 0);
    ck_assert(ziplist_remove((ziplist_p)buf, -1, 1) == ZIPLIST_OK);
    ck_assert_int_eq(ziplist_nentry((ziplist_p)buf), 0);

    ck_assert(ziplist_remove(NULL, 0, 1) == ZIPLIST_ERROR);
    ck_assert(ziplist_remove((ziplist_p)buf, 0, 0) == ZIPLIST_OK);
    ziplist_insert((ziplist_p)buf, &ze_examples[0].decoded, 0);
    ck_assert(ziplist_remove((ziplist_p)buf, 1, 1) == ZIPLIST_EOOB);
    ck_assert(ziplist_remove((ziplist_p)buf, 0, 3) == ZIPLIST_EOOB);
    ck_assert(ziplist_remove((ziplist_p)buf, 0, -2) == ZIPLIST_EOOB);

    /* remove: multiple entry, negative count */
    cc_memcpy(buf, ref, ziplist_size((ziplist_p)ref));
    ck_assert(ziplist_remove((ziplist_p)buf, -1, -n_ze) == ZIPLIST_OK);

}
END_TEST

START_TEST(test_ziplist_removeval)
{
    uint32_t removed;
    int64_t idx;

    /* make buf a double copy of entries in ref */
    cc_memcpy(buf, ref, ziplist_size((ziplist_p)ref));
    cc_memcpy(buf + ziplist_size((ziplist_p)ref), ref + ZIPLIST_HEADER_SIZE,
                ziplist_size((ziplist_p)ref) - ZIPLIST_HEADER_SIZE);
    ZL_NENTRY(buf) = ZL_NENTRY(ref) * 2;
    ZL_NEND(buf) = ZL_NEND(ref) * 2 - ZIPLIST_HEADER_SIZE + 1;

    /* remove both occurrences */
    ck_assert(ziplist_remove_val(&removed, (ziplist_p)buf,
                &ze_examples[0].decoded, n_ze) == ZIPLIST_OK);
    ck_assert_int_eq(removed, 2);
    ck_assert(ziplist_find(NULL, NULL, (ziplist_p)buf, &ze_examples[0].decoded)
            == ZIPLIST_ENOTFOUND);

    /* remove first occurrence */
    ck_assert(ziplist_remove_val(&removed, (ziplist_p)buf,
                &ze_examples[1].decoded, 1) == ZIPLIST_OK);
    ck_assert_int_eq(removed, 1);
    ck_assert(ziplist_find(NULL, &idx, (ziplist_p)buf, &ze_examples[1].decoded)
            == ZIPLIST_OK);
    ck_assert_int_eq(idx, n_ze - 2);

    /* remove last occurrence */
    ck_assert(ziplist_remove_val(&removed, (ziplist_p)buf,
                &ze_examples[2].decoded, -1) == ZIPLIST_OK);
    ck_assert_int_eq(removed, 1);
    ck_assert(ziplist_find(NULL, &idx, (ziplist_p)buf, &ze_examples[2].decoded)
            == ZIPLIST_OK);
    ck_assert_int_eq(idx, 0);

    ck_assert(ziplist_remove_val(&removed, (ziplist_p)buf,
                &ze_examples[3].decoded, -n_ze) == ZIPLIST_OK);
    ck_assert_int_eq(removed, 2);
    ck_assert(ziplist_find(NULL, NULL, (ziplist_p)buf, &ze_examples[3].decoded)
            == ZIPLIST_ENOTFOUND);

}
END_TEST

START_TEST(test_ziplist_truncate_forward_basic)
{
#define CNT 3
    ziplist_p zl = (ziplist_p)buf;
    uint32_t i;

    /* make a copy of ref ziplist */
    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_truncate(zl, CNT), ZIPLIST_OK);

    /* check nentry */
    ck_assert_uint_eq(ziplist_nentry(zl), ziplist_nentry((ziplist_p)ref) - CNT);

    /* check each zipentry */
    for (i = 0; i < ziplist_nentry(zl); ++i) {
        zipentry_p ze;
        ck_assert_int_eq(ziplist_locate(&ze, zl, i), ZIPLIST_OK);
        ck_assert_int_eq(zipentry_compare(ze, &ze_examples[CNT + i].decoded), 0);
    }
#undef CNT
}
END_TEST

START_TEST(test_ziplist_truncate_backward_basic)
{
#define CNT 4
    ziplist_p zl = (ziplist_p)buf;
    uint32_t i;

    /* make a copy of ref ziplist */
    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_truncate(zl, -CNT), ZIPLIST_OK);

    /* check nentry */
    ck_assert_uint_eq(ziplist_nentry(zl), ziplist_nentry((ziplist_p)ref) - CNT);

    /* check each zipentry */
    for (i = 0; i < ziplist_nentry(zl); ++i) {
        zipentry_p ze;
        ck_assert_int_eq(ziplist_locate(&ze, zl, i), ZIPLIST_OK);
        ck_assert_int_eq(zipentry_compare(ze, &ze_examples[i].decoded), 0);
    }
#undef CNT
}
END_TEST

START_TEST(test_ziplist_truncate_overflow)
{
#define CNT 1000
    ziplist_p zl = (ziplist_p)buf;

    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_truncate(zl, CNT), ZIPLIST_OK);
    ck_assert_int_eq(ziplist_nentry(zl), 0);

    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_truncate(zl, -CNT), ZIPLIST_OK);
    ck_assert_int_eq(ziplist_nentry(zl), 0);
#undef CNT
}
END_TEST

static inline void
_get_ref_idx_cnt(uint32_t *ref_idx, uint32_t *ref_cnt, int64_t idx, int64_t cnt)
{
    ck_assert_ptr_ne(ref_idx, NULL);
    ck_assert_ptr_ne(ref_cnt, NULL);

    if (idx >= 0) {
        if (cnt > 0) {
            /* positive idx, counting forward */
            *ref_idx = idx;
            *ref_cnt = idx + cnt > n_ze ? n_ze - idx : cnt;
        } else {
            /* positive idx, counting backward */
            *ref_idx = idx > -cnt ? idx + cnt : 0;
            *ref_cnt = idx > -cnt ? -cnt : idx;
        }
    } else {
        if (cnt > 0) {
            /* negative idx, counting forward */
            *ref_idx = n_ze + idx;
            *ref_cnt = -idx > cnt ? cnt : -idx;
        } else {
            /* negative idx, counting backward */
            *ref_idx = n_ze + idx > -cnt ? n_ze + idx + cnt : 0;
            *ref_cnt = n_ze + idx > -cnt ? -cnt : n_ze + idx;
        }
    }
}

static inline void
_test_ziplist_trim(int64_t idx, int64_t cnt)
{
    ziplist_p zl = (ziplist_p)buf;
    uint32_t i, ref_idx, ref_cnt;

    /* calculate where trimmed list should begin
       on ref list and how far it should go */
    _get_ref_idx_cnt(&ref_idx, &ref_cnt, idx, cnt);

    /* make a copy of ref ziplist */
    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_trim(zl, idx, cnt), ZIPLIST_OK);

    /* check nentry */
    ck_assert_uint_eq(ziplist_nentry(zl), ref_cnt);

    /* check each zipentry */
    for (i = 0; i < ref_cnt; ++i) {
        zipentry_p ze;

        /* get trimmed list entry */
        ck_assert_int_eq(ziplist_locate(&ze, zl, i), ZIPLIST_OK);

        /* compare entries */
        ck_assert_int_eq(zipentry_compare(ze, &ze_examples[ref_idx + i].decoded), 0);
    }
}

START_TEST(test_ziplist_trim_forward_basic)
{
#define IDX 3
#define CNT 3
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_backward_basic)
{
#define IDX 5
#define CNT -3
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_forward_nidx)
{
#define IDX -6
#define CNT 3
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_backward_nidx)
{
#define IDX -2
#define CNT -3
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_overflow)
{
#define IDX 3
#define CNT 100
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_underflow)
{
#define IDX 5
#define CNT -100
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_empty)
{
#define IDX 3
#define CNT 0
    _test_ziplist_trim(IDX, CNT);
#undef IDX
#undef CNT
}
END_TEST

START_TEST(test_ziplist_trim_oob)
{
    ziplist_p zl = (ziplist_p)buf;

    /* make a copy of ref ziplist */
    cc_memcpy(zl, ref, ziplist_size((ziplist_p)ref));

    ck_assert_int_eq(ziplist_trim(zl, ziplist_nentry(zl) + 1, 1), ZIPLIST_EOOB);
    ck_assert_int_eq(ziplist_trim(zl, -(ziplist_nentry(zl) + 1), 1), ZIPLIST_EOOB);
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
        ze_index[i] = (zipentry_p)(ref + sz);
        cc_memcpy(ref + sz, ze_examples[i].encoded, ze_examples[i].nbyte);
        sz += ze_examples[i].nbyte;
    }

    *((uint32_t *)ref) = n_ze;
    *((uint32_t *)ref + 1) = sz - 1;

    TCase *tc_ziplist = tcase_create("ziplist");
    suite_add_tcase(s, tc_ziplist);
    tcase_add_test(tc_ziplist, test_ziplist_seeklocation);
    tcase_add_test(tc_ziplist, test_ziplist_seekvalue);
    tcase_add_test(tc_ziplist, test_ziplist_resetpushpop);
    tcase_add_test(tc_ziplist, test_ziplist_insertremove);
    tcase_add_test(tc_ziplist, test_ziplist_removeval);
    tcase_add_test(tc_ziplist, test_ziplist_truncate_forward_basic);
    tcase_add_test(tc_ziplist, test_ziplist_truncate_backward_basic);
    tcase_add_test(tc_ziplist, test_ziplist_truncate_overflow);
    tcase_add_test(tc_ziplist, test_ziplist_trim_forward_basic);
    tcase_add_test(tc_ziplist, test_ziplist_trim_backward_basic);
    tcase_add_test(tc_ziplist, test_ziplist_trim_forward_nidx);
    tcase_add_test(tc_ziplist, test_ziplist_trim_backward_nidx);
    tcase_add_test(tc_ziplist, test_ziplist_trim_overflow);
    tcase_add_test(tc_ziplist, test_ziplist_trim_underflow);
    tcase_add_test(tc_ziplist, test_ziplist_trim_empty);
    tcase_add_test(tc_ziplist, test_ziplist_trim_oob);

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
