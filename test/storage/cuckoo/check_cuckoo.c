#include <storage/cuckoo/item.h>
#include <storage/cuckoo/cuckoo.h>

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

void
test_insert_collision(uint32_t policy, bool cas)
{
    struct bstring key;
    struct val val;
    rstatus_t status;
    struct item *it;
    int hits = 0;
    char keystring[CC_UINTMAX_MAXLEN];
    uint64_t i, testval;

    ck_assert_msg(test_reset(policy, cas) == CC_OK,
            "could not reset cuckoo module");

    time_update();
    for (i = 0; i < CUCKOO_NITEM + 1; i++) {
        key.len = sprintf(keystring, "%llu", i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        status = cuckoo_insert(&key, &val, UINT32_MAX - 1);
        ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
                status);
    }

    for (i = 0; i < CUCKOO_NITEM + 1; i++) {
        key.len = sprintf(keystring, "%llu", i);
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
    rstatus_t status;
    struct item *it;
    uint64_t cas1, cas2;

    ck_assert_msg(test_reset(policy, true) == CC_OK,
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
    cas1 = item_cas(it);
    ck_assert_uint_ne(cas1, 0);

    val.vstr.data = VAL2;
    val.vstr.len = sizeof(VAL2) - 1;

    status = cuckoo_update(it, &val, UINT32_MAX - 1);
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
    rstatus_t status;
    struct item *it;
    bool deleted;

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
#define NOW 12345678
    struct bstring key;
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

    now = NOW;
    status = cuckoo_insert(&key, &val, NOW + 1);
    ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
            status);

    it = cuckoo_get(&key);
    ck_assert_msg(it != NULL, "cuckoo_get returned NULL");

    now += 2;

    it = cuckoo_get(&key);
    ck_assert_msg(it == NULL, "cuckoo_get returned not NULL after expiration");
#undef NOW
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
    struct bstring key;
    struct val val;
    rstatus_t status;
    char keystring[30];
    uint64_t i;
    cuckoo_metrics_st metrics;

    cuckoo_teardown();
    status = cuckoo_setup(CUCKOO_ITEM_SIZE, CUCKOO_NITEM, CUCKOO_POLICY_RANDOM, true, &metrics);
    ck_assert_msg(status == CC_OK,
            "could not setup cuckoo module");

#define NOW 12345678
    now = NOW;
    for (i = 0; metrics.item_curr.counter < CUCKOO_NITEM; i++) {
        key.len = sprintf(keystring, "%llu", i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        status = cuckoo_insert(&key, &val, now + 1);
        ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
                status);
    }

    // dict is full, all items will expire in now + 1
    now += 2;
    key.len = sprintf(keystring, "%llu", i);
    key.data = keystring;

    val.type = VAL_TYPE_INT;
    val.vint = i;

    status = cuckoo_insert(&key, &val, now + 1);
    ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
            status);
    ck_assert_int_eq(metrics.item_expire.counter, 1);
#undef NOW
}
END_TEST

START_TEST(test_insert_insert_expire_swap)
{
    struct bstring key;
    struct val val;
    rstatus_t status;
    char keystring[30];
    uint64_t i;
    cuckoo_metrics_st metrics;
    int hits = 0;

    cuckoo_teardown();
    status = cuckoo_setup(CUCKOO_ITEM_SIZE, CUCKOO_NITEM, CUCKOO_POLICY_EXPIRE, false, &metrics);
    ck_assert_msg(status == CC_OK,
            "could not setup cuckoo module");

#define NOW 12345678
    now = NOW;
    for (i = 0; metrics.item_curr.counter < CUCKOO_NITEM; i++) {
        key.len = sprintf(keystring, "%llu", i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        status = cuckoo_insert(&key, &val, now + i);
        ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
                status);
    }

    key.len = sprintf(keystring, "%llu", i);
    key.data = keystring;

    val.type = VAL_TYPE_INT;
    val.vint = i;

    status = cuckoo_insert(&key, &val, now + i);
    ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
            status);

    for (;i > 0 && hits < CUCKOO_NITEM;i--) {
        if (cuckoo_get(&key) != NULL) {
            hits++;
        }
    }
    ck_assert_msg(hits == CUCKOO_NITEM, "expected %d hits, got %d",
            CUCKOO_NITEM, hits);
#undef NOW
}
END_TEST

START_TEST(test_expire_displace)
{
    // The goal of this test is to exercise the expiration of keys when
    // displacing a key to insert a new one.
    //
    // To do so, it fills the dictionary with keys, then expire some of them
    // and then adds new keys. All of the new keys' hash will collide with
    // the existing ones (because the dictionary is full) and it will check
    // for expired items in order to displace, all will exist but some will
    // be expired and the key can now be displaced.
#define KEY "key"
#define VAL "value"
#define NOW 12345678
    struct bstring key;
    struct val val;
    rstatus_t status;
    uint64_t i, j;
    char keystring[30];
    int hits = 0;

    cuckoo_metrics_st metrics;

    cuckoo_teardown();
    status = cuckoo_setup(CUCKOO_ITEM_SIZE, CUCKOO_NITEM, CUCKOO_POLICY_RANDOM, true, &metrics);
    ck_assert_msg(status == CC_OK,
            "could not setup cuckoo module");

    now = NOW;
    for (i = 0; metrics.item_curr.counter < CUCKOO_NITEM; i++) {
        key.len = sprintf(keystring, "%llu", i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = i;

        status = cuckoo_insert(&key, &val, now + i);
        ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
                status);
    }

    now += i / 2;
    for (j = 0; j < i; j++) {
        key.len = sprintf(keystring, "%llu", j + i);
        key.data = keystring;

        val.type = VAL_TYPE_INT;
        val.vint = j + i;

        status = cuckoo_insert(&key, &val, now + i);
        ck_assert_msg(status == CC_OK, "cuckoo_insert not OK - return status %d",
                status);
    }

    for (i *= 2 ;i > 0 && hits < CUCKOO_NITEM;i--) {
        if (cuckoo_get(&key) != NULL) {
            hits++;
        }
    }

    ck_assert_msg(hits == CUCKOO_NITEM, "expected %d hits, got %d",
            CUCKOO_NITEM, hits);
#undef NOW
#undef KEY
#undef VAL
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
    tcase_add_test(tc_basic_req, test_expire_displace);

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
