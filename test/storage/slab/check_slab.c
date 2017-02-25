#include <storage/slab/item.h>
#include <storage/slab/slab.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <stdio.h>
#include <string.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "slab"
#define DEBUG_LOG  SUITE_NAME ".log"

slab_options_st options = { SLAB_OPTION(OPTION_INIT) };
slab_metrics_st metrics = { SLAB_METRIC(METRIC_INIT) };

/*
 * utilities
 */
static void
test_setup(void)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    slab_setup(&options, &metrics);
}

static void
test_teardown(void)
{
    slab_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
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

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    ck_assert_msg(!it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, sizeof(VAL) - 1);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(cc_memcmp(item_data(it), VAL, val.len), 0);

    item_insert(it, &key);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
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
#define VLEN (1000 * KiB)

    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;
    size_t len;
    char *p;

    test_reset();

    key = str2bstr(KEY);

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, VLEN);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);

    for (p = item_data(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", VLEN, item_data(it));

#undef VLEN
#undef KEY
}
END_TEST

/**
 * Tests item_reserve, item_backfill and item_release
 */
START_TEST(test_reserve_backfill_release)
{
#define KEY "key"
#define VLEN (1000 * KiB)

    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t vlen, dataflag = 12345;
    size_t len;
    char *p;

    test_reset();

    key = str2bstr(KEY);

    vlen = VLEN;
    val.len = vlen / 2 - 3;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'A', val.len);

    /* reserve */
    status = item_reserve(&it, &key, &val, vlen, dataflag, 0);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);

    ck_assert_msg(it != NULL, "item_reserve returned NULL object");
    ck_assert_msg(!it->is_linked, "item linked by mistake");
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len,
            key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len,
            key.data);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->dataflag, dataflag);
    ck_assert_int_eq(it->vlen, val.len);
    for (p = item_data(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", it->vlen,
            item_data(it));

    /* backfill */
    val.len = vlen - val.len;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'B', val.len);
    item_backfill(it, &val);
    free(val.data);
    ck_assert_msg(!it->is_linked, "item linked by mistake");
    ck_assert_int_eq(it->vlen, vlen);
    for (p = item_data(it) + vlen - val.len, len = val.len;
            len > 0 && *p == 'B'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", val.len,
            item_data(it) + vlen - val.len);

    /* release */
    item_release(&it);
#undef VLEN
#undef KEY
}
END_TEST

START_TEST(test_reserve_backfill_link)
{
#define KEY "key"
#define VLEN (1000 * KiB)

    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    uint32_t dataflag = 12345;
    size_t len;
    char *p;

    test_reset();

    key = str2bstr(KEY);

    val.len = VLEN;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'A', val.len);

    /* reserve */
    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);

    /* backfill & link */
    val.len = 0;
    item_backfill(it, &val);
    item_insert(it, &key);
    ck_assert_msg(it->is_linked, "completely backfilled item not linked");
    ck_assert_int_eq(it->vlen, VLEN);

    for (p = item_data(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", VLEN, item_data(it));

#undef VLEN
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

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);
    append = str2bstr(APPEND);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &append, true);
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

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);
    prepend = str2bstr(PREPEND);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &prepend, false);
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

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    prepend = str2bstr(PREPEND);
    append1 = str2bstr(APPEND1);
    append2 = str2bstr(APPEND2);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &append1, true);
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

    status = item_annex(it, &key, &prepend, false);
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

    status = item_annex(it, &key, &append2, true);
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

    test_reset();

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

    time_update();
    status = item_reserve(&it, &key, &old_val, old_val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    item_update(it, &new_val);

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

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

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

    test_reset();

    key1 = str2bstr(KEY1);
    val1 = str2bstr(VAL1);

    key2 = str2bstr(KEY2);
    val2 = str2bstr(VAL2);

    time_update();
    status = item_reserve(&it, &key1, &val1, val1.len, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key1);

    time_update();
    status = item_reserve(&it, &key2, &val2, val2.len, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key2);

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


START_TEST(test_evict_lru_basic)
{
#define MY_SLAB_SIZE 160
#define MY_SLAB_MAXBYTES 160
    /**
     * These are the slabs that will be created with these parameters:
     *
     * slab size 160, slab hdr size 36, item hdr size 40, item chunk size44, total memory 320
     * class   1: items       2  size      48  data       8  slack      28
     * class   2: items       1  size     120  data      80  slack       4
     *
     * If we use 8 bytes of key+value, it will use the class 1 that can fit
     * two elements. The third one will cause a full slab eviction.
     *
     **/
#define KEY_LENGTH 2
#define VALUE_LENGTH 8
#define NUM_ITEMS 2

    size_t i;
    struct bstring key[NUM_ITEMS + 1] = {
        {KEY_LENGTH, "aa"},
        {KEY_LENGTH, "bb"},
        {KEY_LENGTH, "cc"},
    };
    struct bstring val[NUM_ITEMS + 1] = {
        {VALUE_LENGTH, "aaaaaaaa"},
        {VALUE_LENGTH, "bbbbbbbb"},
        {VALUE_LENGTH, "cccccccc"},
    };
    item_rstatus_t status;
    struct item *it;

    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_size.val.vuint = MY_SLAB_SIZE;
    options.slab_mem.val.vuint = MY_SLAB_MAXBYTES;
    options.slab_evict_opt.val.vuint = EVICT_CS;
    options.slab_item_max.val.vuint = MY_SLAB_SIZE - SLAB_HDR_SIZE;

    test_teardown();
    slab_setup(&options, &metrics);

    for (i = 0; i < NUM_ITEMS + 1; i++) {
        time_update();
        status = item_reserve(&it, &key[i], &val[i], val[i].len, 0, 0);
        ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
        item_insert(it, &key[i]);
        ck_assert_msg(item_get(&key[i]) != NULL, "item %lu not found", i);
    }

    ck_assert_msg(item_get(&key[0]) == NULL,
        "item 0 found, expected to be evicted");
    ck_assert_msg(item_get(&key[1]) == NULL,
        "item 1 found, expected to be evicted");
    ck_assert_msg(item_get(&key[2]) != NULL,
        "item 2 not found");

#undef KEY_LENGTH
#undef VALUE_LENGTH
#undef NUM_ITEMS
#undef MY_SLAB_SIZE
#undef MY_SLAB_MAXBYTES
}
END_TEST

START_TEST(test_refcount)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_t status;
    struct item *it;
    struct slab * s;
    uint32_t dataflag = 12345;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    /* reserve & release */
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    s = item_to_slab(it);
    ck_assert_msg(s->refcount == 1, "slab refcount %"PRIu32"; 1 expected", s->refcount);
    item_release(&it);
    ck_assert_msg(s->refcount == 0, "slab refcount %"PRIu32"; 0 expected", s->refcount);

    /* reserve & backfill (& link) */
    status = item_reserve(&it, &key, &val, val.len, dataflag, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    s = item_to_slab(it);
    ck_assert_msg(s->refcount == 1, "slab refcount %"PRIu32"; 1 expected", s->refcount);
    val = null_bstring;
    item_backfill(it, &val);
    item_insert(it, &key);
    ck_assert_msg(s->refcount == 0, "slab refcount %"PRIu32"; 0 expected", s->refcount);
}
END_TEST

START_TEST(test_evict_refcount)
{
#define MY_SLAB_SIZE 96
#define MY_SLAB_MAXBYTES 96
    /**
     * The slab will be created with these parameters:
     *   slab size 96, slab hdr size 36, item hdr size 40
     * Given that cas 8,
     * we know: key + val < 12
     *
     **/
#define KEY "key"
#define VAL "val"

    struct bstring key, val;
    item_rstatus_t status;
    struct item *it, *nit;

    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_size.val.vuint = MY_SLAB_SIZE;
    options.slab_mem.val.vuint = MY_SLAB_MAXBYTES;
    options.slab_evict_opt.val.vuint = EVICT_CS;
    options.slab_item_max.val.vuint = MY_SLAB_SIZE - SLAB_HDR_SIZE;

    test_teardown();
    slab_setup(&options, &metrics);
    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    status = item_reserve(&nit, &key, &val, val.len, 0, 0);
    ck_assert_msg(status == ITEM_ENOMEM, "item_reserve should fail - return status %d", status);

    item_insert(it, &key); /* clears slab refcount, can be evicted */
    status = item_reserve(&nit, &key, &val, val.len, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);

#undef KEY
#undef VAL
#undef MY_SLAB_SIZE
#undef MY_SLAB_MAXBYTES
    test_reset();

    /* reserve & release */
}
END_TEST

/*
 * test suite
 */
static Suite *
slab_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic item */
    TCase *tc_item = tcase_create("item api");
    suite_add_tcase(s, tc_item);

    tcase_add_test(tc_item, test_insert_basic);
    tcase_add_test(tc_item, test_insert_large);
    tcase_add_test(tc_item, test_reserve_backfill_release);
    tcase_add_test(tc_item, test_reserve_backfill_link);
    tcase_add_test(tc_item, test_append_basic);
    tcase_add_test(tc_item, test_prepend_basic);
    tcase_add_test(tc_item, test_annex_sequence);
    tcase_add_test(tc_item, test_delete_basic);
    tcase_add_test(tc_item, test_update_basic);
    tcase_add_test(tc_item, test_flush_basic);

    TCase *tc_slab = tcase_create("slab api");
    suite_add_tcase(s, tc_slab);
    tcase_add_test(tc_slab, test_evict_lru_basic);
    tcase_add_test(tc_slab, test_refcount);
    tcase_add_test(tc_slab, test_evict_refcount);

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
