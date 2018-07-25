#include <storage/cuckoo/item.h>
#include <storage/cuckoo/cuckoo.h>

#include <time/time.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <string.h>
#include <stdio.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "cuckoo"
#define DEBUG_LOG  SUITE_NAME ".log"

void test_insert_basic(uint32_t policy, bool cas);
void test_insert_collision(uint32_t policy, bool cas);
void test_cas(uint32_t policy);
void test_delete_basic(uint32_t policy, bool cas);
void test_expire_basic(uint32_t policy, bool cas);

cuckoo_options_st options = { CUCKOO_OPTION(OPTION_INIT) };
cuckoo_metrics_st metrics = { CUCKOO_METRIC(METRIC_INIT) };

/*
 * utilities
 */
static void
test_setup(uint32_t policy, bool cas)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.cuckoo_policy.val.vuint = policy;
    options.cuckoo_item_cas.val.vbool = cas;

    cuckoo_setup(&options, &metrics);
}

static void
test_teardown(void)
{
    cuckoo_teardown();
}

static void
test_reset(uint32_t policy, bool cas)
{
    test_teardown();
    test_setup(policy, cas);
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
    struct item *it;

    test_reset(policy, cas);

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.type = VAL_TYPE_STR;
    val.vstr.data = VAL;
    val.vstr.len = sizeof(VAL) - 1;

    time_update();
    it = cuckoo_insert(&key, &val, INT32_MAX);
    ck_assert_msg(it != NULL, "cuckoo_insert not OK");

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

void
test_insert_collision(uint32_t policy, bool cas)
{
    struct bstring key;
    struct val val;
    struct item *it;
    int hits = 0;
    char keystring[CC_UINTMAX_MAXLEN];
    uint64_t i, testval;

    test_reset(policy, cas);

    time_update();
    for (i = 0; i < CUCKOO_NITEM + 1; i++) {
        key.len = sprintf(keystring, "%"PRIu64, i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        it = cuckoo_insert(&key, &val, INT32_MAX);
        ck_assert_msg(it != NULL, "cuckoo_insert not OK");
    }

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

void
test_cas(uint32_t policy)
{
#define KEY "key"
#define VAL "value"
#define VAL2 "value2"
    struct bstring key;
    struct val val;
    rstatus_i status;
    struct item *it;
    uint64_t cas1, cas2;

    test_reset(policy, true);

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.type = VAL_TYPE_STR;
    val.vstr.data = VAL;
    val.vstr.len = sizeof(VAL) - 1;

    time_update();
    it = cuckoo_insert(&key, &val, INT32_MAX);
    ck_assert_msg(it != NULL, "cuckoo_insert not OK");

    it = cuckoo_get(&key);
    cas1 = item_cas(it);
    ck_assert_uint_ne(cas1, 0);

    val.vstr.data = VAL2;
    val.vstr.len = sizeof(VAL2) - 1;

    status = cuckoo_update(it, &val, INT32_MAX);
    ck_assert_msg(status == CC_OK, "cuckoo_update not OK - return status %d",
            status);

    it = cuckoo_get(&key);
    cas2 = item_cas(it);
    ck_assert_uint_ne(cas2, 0);
    ck_assert_uint_ne(cas1, cas2);
#undef KEY
#undef VAL
#undef VAL2
}

void
test_delete_basic(uint32_t policy, bool cas)
{
#define KEY "key"
#define VAL "value"
    struct bstring key;
    struct val val;
    struct item *it;
    bool deleted;

    test_reset(policy, cas);

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.type = VAL_TYPE_STR;
    val.vstr.data = VAL;
    val.vstr.len = sizeof(VAL) - 1;

    time_update();
    it = cuckoo_insert(&key, &val, INT32_MAX);
    ck_assert_msg(it != NULL, "cuckoo_insert not OK");

    it = cuckoo_get(&key);
    ck_assert_msg(it != NULL, "cuckoo_get returned NULL");

    deleted = cuckoo_delete(&key);
    ck_assert_msg(deleted, "cuckoo_delete return false");

    it = cuckoo_get(&key);
    ck_assert_msg(it == NULL, "cuckoo_get returned not NULL");

    deleted = cuckoo_delete(&key);
    ck_assert_msg(!deleted, "cuckoo_delete return true");
#undef KEY
#undef VAL
}

void
test_expire_basic(uint32_t policy, bool cas)
{
#define KEY "key"
#define VAL "value"
#define TIME 12345678
    struct bstring key;
    struct val val;
    struct item *it;

    test_reset(policy, cas);

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.type = VAL_TYPE_STR;
    val.vstr.data = VAL;
    val.vstr.len = sizeof(VAL) - 1;

    proc_sec = TIME;
    it = cuckoo_insert(&key, &val, TIME + 1);
    ck_assert_msg(it != NULL, "cuckoo_insert not OK");

    it = cuckoo_get(&key);
    ck_assert_msg(it != NULL, "cuckoo_get returned NULL");

    proc_sec += 2;

    it = cuckoo_get(&key);
    ck_assert_msg(it == NULL, "cuckoo_get returned not NULL after expiration");
#undef TIME
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

START_TEST(test_insert_collision_random_true)
{
    test_insert_collision(CUCKOO_POLICY_RANDOM, true);
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

START_TEST(test_insert_collision_expire_false)
{
    test_insert_collision(CUCKOO_POLICY_EXPIRE, false);
}
END_TEST

START_TEST(test_cas_random)
{
    test_cas(CUCKOO_POLICY_RANDOM);
}
END_TEST

START_TEST(test_cas_expire)
{
    test_cas(CUCKOO_POLICY_EXPIRE);
}
END_TEST

START_TEST(test_delete_basic_random_true)
{
    test_delete_basic(CUCKOO_POLICY_RANDOM, true);
}
END_TEST

START_TEST(test_delete_basic_random_false)
{
    test_delete_basic(CUCKOO_POLICY_RANDOM, false);
}
END_TEST

START_TEST(test_expire_basic_random_true)
{
    test_expire_basic(CUCKOO_POLICY_RANDOM, true);
}
END_TEST

START_TEST(test_expire_basic_random_false)
{
    test_expire_basic(CUCKOO_POLICY_RANDOM, false);
}
END_TEST

START_TEST(test_insert_replace_expired)
{
#define TIME 12345678

    struct bstring key;
    struct val val;
    char keystring[30];
    uint64_t i;

    metrics = (cuckoo_metrics_st) { CUCKOO_METRIC(METRIC_INIT) };
    test_reset(CUCKOO_POLICY_EXPIRE, true);

    proc_sec = TIME;
    for (i = 0; metrics.item_curr.counter < CUCKOO_NITEM; i++) {
        key.len = sprintf(keystring, "%"PRIu64, i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        ck_assert_msg(cuckoo_insert(&key, &val, proc_sec + 1) != NULL,
                "cuckoo_insert not OK");
    }

    // dict is full, all items will expire in proc_sec + 1
    proc_sec += 2;
    key.len = sprintf(keystring, "%"PRIu64, i);
    key.data = keystring;

    val.type = VAL_TYPE_INT;
    val.vint = i;

    ck_assert_msg(cuckoo_insert(&key, &val, proc_sec + 1) != NULL,
                "cuckoo_insert failed");
    ck_assert_int_eq(metrics.item_expire.counter, 1);
#undef TIME
}
END_TEST

START_TEST(test_insert_insert_expire_swap)
{
#define TIME 12345678
    struct bstring key;
    struct val val;
    char keystring[30];
    uint64_t i;
    int hits = 0;

    metrics = (cuckoo_metrics_st) { CUCKOO_METRIC(METRIC_INIT) };
    test_reset(CUCKOO_POLICY_EXPIRE, false);

    proc_sec = TIME;
    for (i = 0; metrics.item_curr.counter < CUCKOO_NITEM; i++) {
        key.len = sprintf(keystring, "%"PRIu64, i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        ck_assert_msg(cuckoo_insert(&key, &val, proc_sec + i) != NULL,
                "cuckoo_insert not OK");
    }

    key.len = sprintf(keystring, "%"PRIu64, i);
    key.data = keystring;

    val.type = VAL_TYPE_INT;
    val.vint = i;

    ck_assert_msg(cuckoo_insert(&key, &val, proc_sec + i) != NULL,
            "cuckoo_insert not OK");

    for (;i > 0 && hits < CUCKOO_NITEM;i--) {
        if (cuckoo_get(&key) != NULL) {
            hits++;
        }
    }
    ck_assert_msg(hits == CUCKOO_NITEM, "expected %d hits, got %d",
            CUCKOO_NITEM, hits);
#undef TIME
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
    tcase_add_test(tc_basic_req, test_insert_collision_random_true);
    tcase_add_test(tc_basic_req, test_insert_collision_random_false);
    tcase_add_test(tc_basic_req, test_insert_collision_expire_true);
    tcase_add_test(tc_basic_req, test_insert_collision_expire_false);
    tcase_add_test(tc_basic_req, test_cas_random);
    tcase_add_test(tc_basic_req, test_cas_expire);
    tcase_add_test(tc_basic_req, test_delete_basic_random_true);
    tcase_add_test(tc_basic_req, test_delete_basic_random_false);
    tcase_add_test(tc_basic_req, test_expire_basic_random_true);
    tcase_add_test(tc_basic_req, test_expire_basic_random_false);
    tcase_add_test(tc_basic_req, test_insert_replace_expired);
    tcase_add_test(tc_basic_req, test_insert_insert_expire_swap);

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
