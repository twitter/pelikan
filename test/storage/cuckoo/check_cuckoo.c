#include <storage/cuckoo/item.h>
#include <storage/cuckoo/cuckoo.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <string.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "cuckoo"
#define DEBUG_LOG  SUITE_NAME ".log"

void test_insert_basic(uint32_t policy, bool cas);

/*
 * utilities
 */
static rstatus_t
test_setup(uint32_t policy, bool cas)
{
    return cuckoo_setup(CUCKOO_ITEM_SIZE, CUCKOO_NITEM, policy, cas, NULL);
}

static void
test_teardown(void)
{
    cuckoo_teardown();
}

static rstatus_t
test_reset(uint32_t policy, bool cas)
{
    test_teardown();
    return test_setup(policy, cas);
}

/**
 * Tests basic functionality for cuckoo_insert and cuckoo_get with small key/val. Checks that the
 * commands succeed and that the item returned is well-formed.
 */
void
test_insert_basic(uint32_t policy, bool cas)
{
#define KEY "key"
#define VAL "value"
    struct bstring key, testval;
    struct val val;
    rstatus_t status;
    struct item *it;

    ck_assert_msg(test_reset(policy, cas) == CC_OK,
            "could not reset cuckoo module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.type = VAL_TYPE_STR;
    val.vstr.data = VAL;
    val.vstr.len = sizeof(VAL) - 1;

    time_update();
    status = cuckoo_insert(&key, &val, UINT32_MAX - 1);
    ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
            status);

    it = cuckoo_get(&key);
    ck_assert_msg(it != NULL, "cuckoo_get returned NULL");
    ck_assert_int_eq(it->vlen, sizeof(VAL) - 1);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    item_value_str(&testval, it);
    ck_assert_int_eq(it->vlen, testval.len);
    ck_assert_int_eq(cc_memcmp(testval.data, VAL, testval.len), 0);
#undef KEY
#undef VAL
}

START_TEST(test_insert_basic_random_true)
{
    test_insert_basic(CUCKOO_POLICY_RANDOM, true);
}
END_TEST

START_TEST(test_insert_basic_random_false)
{
    test_insert_basic(CUCKOO_POLICY_RANDOM, false);
}
END_TEST

/*
 * test suite
 */
static Suite *
cuckoo_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_basic_req = tcase_create("basic item api");
    suite_add_tcase(s, tc_basic_req);

    test_setup(CUCKOO_POLICY_RANDOM, true);

    tcase_add_test(tc_basic_req, test_insert_basic_random_true);
    tcase_add_test(tc_basic_req, test_insert_basic_random_false);

    return s;
}

int
main(void)
{
    int nfail;

    Suite *suite = cuckoo_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
