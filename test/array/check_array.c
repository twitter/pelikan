#include <cc_array.h>

#include <check.h>

#include <stdlib.h>
#include <stdio.h>

#define SUITE_NAME "array"
#define DEBUG_LOG  SUITE_NAME ".log"

#define ARRAY_MAX_NELEM_DELTA 8

/*
 * utilities
 */
static void
test_setup(void)
{
    array_options_st options = {.array_nelem_delta = {.set = true,
        .type = OPTION_TYPE_UINT, .val.vuint = ARRAY_MAX_NELEM_DELTA}};
    array_setup(&options);
}

static void
test_teardown(void)
{
    array_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

static void
_test_create_push_pop_destroy(uint32_t initial_nalloc, uint32_t times, uint32_t expected_nalloc)
{
#define SIZE 8
    struct array *arr;
    uint32_t i;
    uint64_t *el;

    test_reset();

    ck_assert_int_eq(array_create(&arr, initial_nalloc, SIZE), CC_OK);
    ck_assert_ptr_ne(arr, NULL);
    ck_assert_int_eq(array_nalloc(arr), initial_nalloc);

    for (i = 0; i < times; i++) {
        el = array_push(arr);
        *el = i;
        ck_assert_int_eq(array_nelem(arr), i + 1);
    }

    ck_assert_int_eq(array_nalloc(arr), expected_nalloc);

    for (i = times - 1; ; i--) {
        el = array_pop(arr);
        ck_assert_int_eq(*el, i);
        ck_assert_int_eq(array_nelem(arr), i);
        if (i == 0) {
            break;
        }
    }

    array_destroy(&arr);
    ck_assert_ptr_eq(arr, NULL);
#undef SIZE
}

START_TEST(test_create_push_pop_destroy)
{
    _test_create_push_pop_destroy(4, 3, 4);
}
END_TEST

START_TEST(test_expand)
{
    _test_create_push_pop_destroy(2, 3, 4);
}
END_TEST

START_TEST(test_expand_max)
{
    _test_create_push_pop_destroy(ARRAY_MAX_NELEM_DELTA + 1,
            ARRAY_MAX_NELEM_DELTA + 2, ARRAY_MAX_NELEM_DELTA * 2 + 1);
}
END_TEST

static rstatus_i
sum(void *_elem, void *_agg)
{
    *(uint64_t*)_agg = *(uint64_t*)_agg + *(uint64_t*)_elem;
    return CC_OK;
}

START_TEST(test_each)
{
#define SIZE 8
#define NELEM 16
    struct array *arr;
    uint32_t i;
    uint64_t *el;
    uint64_t agg = 0;
    uint64_t agg2 = 0;
    err_i err = 0;

    test_reset();

    ck_assert_int_eq(array_create(&arr, NELEM, SIZE), CC_OK);

    for (i = 0; i < NELEM; i++) {
        el = array_push(arr);
        *el = i;
        agg2 += i;
    }
    array_each(arr, sum, &agg, &err);
    ck_assert_int_eq(err, 0);
    ck_assert_int_eq(agg, agg2);

    array_destroy(&arr);
#undef NELEM
#undef SIZE
}
END_TEST

static int cmp(const void *e1, const void *e2) {
    return memcmp(e1, e2, sizeof(uint64_t));
}

START_TEST(test_sort)
{
#define SIZE 8
#define NELEM 16
    struct array *arr;
    uint32_t i;
    uint64_t *el;

    test_reset();

    ck_assert_int_eq(array_create(&arr, NELEM, SIZE), CC_OK);

    /* array: [NELEM, NELEM - 1, ...., 1] */
    for (i = 0; i < NELEM; i++) {
        el = array_push(arr);
        *el = NELEM - i;
    }
    array_sort(arr, cmp);

    for (; i > 0; i--) {
        el = array_pop(arr);
        ck_assert_int_eq(*el, i);
    }

    array_destroy(&arr);
#undef NELEM
#undef SIZE
}
END_TEST

/*
 * test suite
 */
static Suite *
array_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_array = tcase_create("cc_array test");
    suite_add_tcase(s, tc_array);

    tcase_add_test(tc_array, test_create_push_pop_destroy);
    tcase_add_test(tc_array, test_expand);
    tcase_add_test(tc_array, test_expand_max);
    tcase_add_test(tc_array, test_each);
    tcase_add_test(tc_array, test_sort);

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

    Suite *suite = array_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
