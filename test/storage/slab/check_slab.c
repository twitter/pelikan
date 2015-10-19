#include <storage/slab/item.h>
#include <storage/slab/slab.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <string.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "slab"
#define DEBUG_LOG  SUITE_NAME ".log"

/*
 * utilities
 */
static rstatus_t
test_setup(void)
{
    rstatus_t status;

    status = slab_setup(SLAB_SIZE, true, EVICT_NONE, true, SLAB_MIN_CHUNK,
                        SLAB_MAX_CHUNK, GiB, NULL, str(SLAB_FACTOR), NULL);
    if (status != CC_OK) {
        return status;
    }

    return item_setup(true, HASH_POWER, NULL);
}

static void
test_teardown(void)
{
    item_teardown();
    slab_teardown();
}

static rstatus_t
test_reset(void)
{
    test_teardown();
    return test_setup();
}

/**
 * Tests basic functionality for item_insert and item_get with small key/val. Checks that the
 * commands succeed and that the item returned is well-formed.
 */
START_TEST(test_insert_basic)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = VAL;
    val.len = sizeof(VAL) - 1;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, sizeof(VAL) - 1);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), VAL, val.len), 0);
#undef KEY
#undef VAL
}
END_TEST

/**
 * Tests item_insert and item_get for large value (close to 1 MiB). Checks that the commands
 * succeed and that the item returned is well-formed.
 */
START_TEST(test_insert_large)
{
#define KEY "key"
    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = cc_alloc(1000 * KiB);
    cc_memset(val.data, 'A', 1000 * KiB);
    val.data[1000 * KiB - 1] = '\0';
    val.len = 1000 * KiB;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, 1000 * KiB);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_msg(strspn(item_data(it), "A") == 1000 * KiB - 1, "item_data contains wrong value %.*s", 1000 * KiB, item_data(it));
#undef KEY
}
END_TEST

/**
 * Tests basic append functionality for item_annex.
 */
START_TEST(test_append_basic)
{
#define KEY "key"
#define VAL "val"
#define APPEND "append"
    struct bstring key, val, append;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = VAL;
    val.len = sizeof(VAL) - 1;

    append.data = APPEND;
    append.len = sizeof(APPEND) - 1;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &append, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + append.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), VAL APPEND, val.len + append.len), 0);
#undef KEY
#undef VAL
#undef APPEND
}
END_TEST

/**
 * Tests basic prepend functionality for item_annex.
 */
START_TEST(test_prepend_basic)
{
#define KEY "key"
#define VAL "val"
#define PREPEND "prepend"
    struct bstring key, val, prepend;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = VAL;
    val.len = sizeof(VAL) - 1;

    prepend.data = PREPEND;
    prepend.len = sizeof(PREPEND) - 1;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &prepend, false);
    ck_assert_msg(status == ITEM_OK, "item_prepend not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(it->is_raligned, "item with key %.*s is not raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + prepend.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), PREPEND VAL, val.len + prepend.len), 0);
#undef KEY
#undef VAL
#undef PREPEND
}
END_TEST

/**
 * Tests append followed by prepend followed by append. Checks for alignment.
 */
START_TEST(test_annex_sequence)
{
#define KEY "key"
#define VAL "val"
#define PREPEND "prepend"
#define APPEND1 "append1"
#define APPEND2 "append2"
    struct bstring key, val, prepend, append1, append2;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = VAL;
    val.len = sizeof(VAL) - 1;

    prepend.data = PREPEND;
    prepend.len = sizeof(PREPEND) - 1;

    append1.data = APPEND1;
    append1.len = sizeof(APPEND1) - 1;

    append2.data = APPEND2;
    append2.len = sizeof(APPEND2) - 1;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &append1, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + append1.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), VAL APPEND1, val.len + append1.len), 0);

    status = item_annex(it, &prepend, false);
    ck_assert_msg(status == ITEM_OK, "item_prepend not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(it->is_raligned, "item with key %.*s is not raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + append1.len + prepend.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), PREPEND VAL APPEND1, val.len + append1.len + prepend.len), 0);

    status = item_annex(it, &append2, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + append1.len + prepend.len + append2.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), PREPEND VAL APPEND1 APPEND2, val.len + append1.len + prepend.len + append2.len), 0);
#undef KEY
#undef VAL
#undef PREPEND
#undef APPEND1
#undef APPEND2
}
END_TEST

/**
 * Tests basic functionality for item_update
 */
START_TEST(test_update_basic)
{
#define KEY "key"
#define OLD_VAL "old_val"
#define NEW_VAL "new_val"
    struct bstring key, old_val, new_val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    old_val.data = OLD_VAL;
    old_val.len = sizeof(OLD_VAL) - 1;

    new_val.data = NEW_VAL;
    new_val.len = sizeof(NEW_VAL) - 1;

    time_update();
    status = item_insert(&key, &old_val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_update(it, &new_val);
    ck_assert_msg(status == CC_OK, "item_update not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, new_val.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), NEW_VAL, new_val.len), 0);
#undef KEY
#undef OLD_VAL
#undef NEW_VAL
}
END_TEST

/**
 * Tests basic functionality for item_delete
 */
START_TEST(test_delete_basic)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key.data = KEY;
    key.len = sizeof(KEY) - 1;

    val.data = VAL;
    val.len = sizeof(VAL) - 1;

    time_update();
    status = item_insert(&key, &val, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    ck_assert_msg(item_delete(&key), "item_delete for key %.*s not successful", key.len, key.data);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after delete", key.len, key.data);
#undef KEY
#undef VAL
}
END_TEST

/**
 * Tests basic functionality for item_flush
 */
START_TEST(test_flush_basic)
{
#define KEY1 "key1"
#define VAL1 "val1"
#define KEY2 "key2"
#define VAL2 "val2"
    struct bstring key1, val1, key2, val2;
    item_rstatus_t status;
    struct item *it;

    ck_assert_msg(test_reset() == CC_OK, "could not reset slab module");

    key1.data = KEY1;
    key1.len = sizeof(KEY1) - 1;

    val1.data = VAL1;
    val1.len = sizeof(VAL1) - 1;

    key2.data = KEY2;
    key2.len = sizeof(KEY2) - 1;

    val2.data = VAL2;
    val2.len = sizeof(VAL2) - 1;

    time_update();
    status = item_insert(&key1, &val1, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    time_update();
    status = item_insert(&key2, &val2, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_insert not OK - return status %d", status);

    item_flush();
    it = item_get(&key1);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush", key1.len, key1.data);
    it = item_get(&key2);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush", key2.len, key2.data);
#undef KEY1
#undef VAL1
#undef KEY2
#undef VAL2
}
END_TEST

/*
 * test suite
 */
static Suite *
slab_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic requests */
    TCase *tc_basic_req = tcase_create("basic item api");
    suite_add_tcase(s, tc_basic_req);

    tcase_add_test(tc_basic_req, test_insert_basic);
    tcase_add_test(tc_basic_req, test_insert_large);
    tcase_add_test(tc_basic_req, test_append_basic);
    tcase_add_test(tc_basic_req, test_prepend_basic);
    tcase_add_test(tc_basic_req, test_annex_sequence);
    tcase_add_test(tc_basic_req, test_delete_basic);
    tcase_add_test(tc_basic_req, test_update_basic);
    tcase_add_test(tc_basic_req, test_flush_basic);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = slab_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
