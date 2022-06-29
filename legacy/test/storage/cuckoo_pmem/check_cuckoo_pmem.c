#include <storage/cuckoo/item.h>
#include <storage/cuckoo/cuckoo.h>

#include <time/time.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <string.h>
#include <stdio.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "cuckoo_pmem"
#define DEBUG_LOG  SUITE_NAME ".log"
#define DATAPOOL_PATH "./datapool.pelikan"

void test_insert_basic(uint32_t policy, bool cas);
void test_insert_collision(uint32_t policy, bool cas);
void test_cas(uint32_t policy);
void test_delete_basic(uint32_t policy, bool cas);
void test_expire_basic(uint32_t policy, bool cas);
void test_expire_truncated(uint32_t policy, bool cas);

cuckoo_options_st options = { CUCKOO_OPTION(OPTION_INIT) };
cuckoo_metrics_st metrics = { CUCKOO_METRIC(METRIC_INIT) };

/*
 * utilities
 */
static void
test_setup(uint32_t policy, bool cas, delta_time_i ttl)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.cuckoo_policy.val.vuint = policy;
    options.cuckoo_item_cas.val.vbool = cas;
    options.cuckoo_max_ttl.val.vuint = ttl;
    options.cuckoo_datapool.val.vstr = DATAPOOL_PATH;

    cuckoo_setup(&options, &metrics);
}

static void
test_teardown(int un)
{
    cuckoo_teardown();
    if (un)
        unlink(DATAPOOL_PATH);
}

static void
test_reset(uint32_t policy, bool cas, delta_time_i ttl, int un)
{
    test_teardown(un);
    test_setup(policy, cas, ttl);
}

static void
test_assert_entry_exists(struct bstring *key, struct val *val)
{
    struct item *it = cuckoo_get(key);
    ck_assert_msg(it != NULL, "cuckoo_get returned NULL");
    ck_assert_int_eq(it->vlen, val->vstr.len);
    ck_assert_int_eq(it->klen, key->len);
    ck_assert_int_eq(it->vlen, val->vstr.len);
    struct bstring testval;
    item_value_str(&testval, it);
    ck_assert_int_eq(it->vlen, testval.len);
    ck_assert_int_eq(cc_memcmp(testval.data, val->vstr.data, testval.len), 0);
}

/**
 * Tests basic functionality for cuckoo_insert and cuckoo_get with small key/val.
 * Checks that the commands succeed and that the item returned is well-formed.
 */
void
test_insert_basic(uint32_t policy, bool cas)
{
#define KEY "key"
#define VAL "value"

    struct bstring key;
    struct val val;
    struct item *it;

    test_reset(policy, cas, CUCKOO_MAX_TTL, 0);

    bstring_set_literal(&key, KEY);

    val.type = VAL_TYPE_STR;
    bstring_set_literal(&val.vstr, VAL);

    time_update();
    it = cuckoo_insert(&key, &val, INT32_MAX);
    ck_assert_msg(it != NULL, "cuckoo_insert not OK");

    test_assert_entry_exists(&key, &val);

    test_reset(policy, cas, CUCKOO_MAX_TTL, 0);

    test_assert_entry_exists(&key, &val);

#undef KEY
#undef VAL
}

void
test_insert_collision(uint32_t policy, bool cas)
{
    struct bstring key;
    struct val val;
    struct item *it;
    int hits = 0;
    char keystring[CC_UINTMAX_MAXLEN];
    uint64_t i, testval;

    test_reset(policy, cas, CUCKOO_MAX_TTL, 1);

    time_update();
    for (i = 0; i < CUCKOO_NITEM + 1; i++) {
        key.len = sprintf(keystring, "%"PRIu64, i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        it = cuckoo_insert(&key, &val, INT32_MAX);
        ck_assert_msg(it != NULL, "cuckoo_insert not OK");
    }

    test_reset(policy, cas, CUCKOO_MAX_TTL, 0);

    for (i = 0; i < CUCKOO_NITEM + 1; i++) {
        key.len = sprintf(keystring, "%"PRIu64, i);
        key.data = keystring;

        it = cuckoo_get(&key);
        if (it == NULL) {
            continue;
        }
        hits++;
        ck_assert_int_eq(it->klen, key.len);
        testval = item_value_int(it);
        ck_assert_int_eq(testval, i);
    }

    ck_assert_msg(hits > (double)CUCKOO_NITEM * 9 / 10, "hit rate is lower than expected when hash collision occurs");
    ck_assert_msg(hits <= CUCKOO_NITEM, "hit rate is too high, expected more evicted values");
}


START_TEST(test_insert_basic_random_true)
{
    test_insert_basic(CUCKOO_POLICY_RANDOM, true);
}
END_TEST

START_TEST(test_insert_basic_expire_true)
{
    test_insert_basic(CUCKOO_POLICY_EXPIRE, true);
}
END_TEST

START_TEST(test_insert_collision_random_false)
{
    test_insert_collision(CUCKOO_POLICY_RANDOM, false);
}
END_TEST

START_TEST(test_insert_collision_expire_true)
{
    test_insert_collision(CUCKOO_POLICY_EXPIRE, true);
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
    TCase *tc_basic = tcase_create("basic");
    suite_add_tcase(s, tc_basic);

    tcase_add_test(tc_basic, test_insert_basic_random_true);
    tcase_add_test(tc_basic, test_insert_basic_expire_true);

    TCase *tc_collision = tcase_create("collision");
    suite_add_tcase(s, tc_collision);

    tcase_add_test(tc_collision, test_insert_collision_random_false);
    tcase_add_test(tc_collision, test_insert_collision_expire_true);

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
    test_teardown(1);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
