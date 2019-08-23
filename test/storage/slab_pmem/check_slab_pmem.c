#include <storage/slab/item.h>
#include <storage/slab/slab.h>

#include <time/time.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <stdio.h>
#include <string.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "slab"
#define DEBUG_LOG  SUITE_NAME ".log"
#define DATAPOOL_PATH "./slab_datapool.pelikan"

slab_options_st options = { SLAB_OPTION(OPTION_INIT) };
slab_metrics_st metrics = { SLAB_METRIC(METRIC_INIT) };

extern delta_time_i max_ttl;

/*
 * utilities
 */
static void
test_setup(void)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_datapool.val.vstr = DATAPOOL_PATH;
    slab_setup(&options, &metrics);
}

static void
test_teardown(int un)
{
    slab_teardown();
    if (un)
        unlink(DATAPOOL_PATH);
}

static void
test_reset(int un)
{
    test_teardown(un);
    test_setup();
}

static void
test_assert_insert_basic_entry_exists(struct bstring key)
{
    struct item *it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, sizeof("val") - 1);
    ck_assert_int_eq(cc_memcmp("val", item_data(it), sizeof("val") - 1), 0);
    ck_assert_int_eq(it->klen, sizeof("key") - 1);
    ck_assert_int_eq(cc_memcmp("key", item_key(it), sizeof("key") - 1), 0);
}

static void
test_assert_insert_large_entry_exists(struct bstring key)
{
    size_t len;
    char *p;
    struct item *it  = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, 1000 * KiB);
    ck_assert_int_eq(it->klen, sizeof("key") - 1);
    ck_assert_int_eq(cc_memcmp("key", item_key(it), sizeof("key") - 1), 0);

    for (p = item_data(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", (1000 * KiB), item_data(it));
}

static void
test_assert_reserve_backfill_link_exists(struct item *it)
{
    size_t len;
    char *p;

    ck_assert_msg(it->is_linked, "completely backfilled item not linked");
    ck_assert_int_eq(it->vlen, (1000 * KiB));

    for (p = item_data(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", (1000 * KiB), item_data(it));
}

static void
test_assert_reserve_backfill_not_linked(struct item *it, size_t pattern_len)
{
    size_t len;
    char *p;

    ck_assert_msg(!it->is_linked, "item linked by mistake");
    ck_assert_int_eq(it->vlen, (1000 * KiB));
    for (p = item_data(it) + it->vlen - pattern_len, len = pattern_len;
            len > 0 && *p == 'B'; p++, len--);
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", pattern_len,
            item_data(it) + it->vlen - pattern_len);
}

static void
test_assert_annex_sequence_exists(struct bstring key, uint32_t len, const char *literal, bool realigned)
{
    struct item *it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(realigned ? !it->is_raligned : it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, len);
    ck_assert_int_eq(it->klen, sizeof("key") - 1);
    ck_assert_int_eq(cc_memcmp(item_data(it), literal, len), 0);
}

static void
test_assert_expire_exists(struct bstring key, proc_time_i sec)
{
    struct item *it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get on unexpired item not successful");

    proc_sec += sec;
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item_get returned not NULL after expiration");
}

static void
test_assert_update_basic_entry_exists(struct bstring key)
{
    struct item *it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, sizeof("new_val") - 1);
    ck_assert_int_eq(it->klen, sizeof("key") - 1);
    ck_assert_int_eq(cc_memcmp(item_data(it), "new_val", sizeof("new_val") - 1), 0);
}

/**
 * Tests basic functionality for item_insert with small key/val. Checks that the
 * commands succeed and that the item returned is well-formed.
 */
START_TEST(test_insert_basic)
{
#define KEY "key"
#define VAL "val"
#define MLEN 8
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, MLEN, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    ck_assert_msg(it != NULL, "item_reserve with key %.*s reserved NULL item",
            key.len, key.data);
    ck_assert_msg(!it->is_linked, "item with key %.*s not linked", key.len,
            key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len,
            key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len,
            key.data);
    ck_assert_int_eq(it->vlen, sizeof(VAL) - 1);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(item_data(it) - (char *)it, offsetof(struct item, end) +
            item_cas_size() + MLEN + sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(item_data(it), VAL, val.len), 0);

    item_insert(it, &key);

    test_assert_insert_basic_entry_exists(key);

    test_reset(0);

    test_assert_insert_basic_entry_exists(key);

#undef MLEN
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
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    test_assert_insert_large_entry_exists(key);

    test_reset(0);

    test_assert_insert_large_entry_exists(key);

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
    item_rstatus_e status;
    struct item *it;
    uint32_t vlen;
    size_t len;
    struct slab *s;
    char *p;

    test_reset(1);

    key = str2bstr(KEY);

    vlen = VLEN;
    val.len = vlen / 2 - 3;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'A', val.len);

    /* reserve */
    status = item_reserve(&it, &key, &val, vlen, 0, INT32_MAX);
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

    s = item_to_slab(it);

    test_assert_reserve_backfill_not_linked(it, val.len);
    ck_assert_msg(s->refcount == 1, "slab refcount %"PRIu32"; 1 expected", s->refcount);
    test_reset(0);

    test_assert_reserve_backfill_not_linked(it, val.len);
    /* check if item was released after reset */
    ck_assert_msg(s->refcount == 0, "slab refcount %"PRIu32"; 0 expected", s->refcount);

#undef VLEN
#undef KEY
}
END_TEST

START_TEST(test_reserve_backfill_link)
{
#define KEY "key"
#define VLEN (1000 * KiB)

    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);

    val.len = VLEN;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'A', val.len);

    /* reserve */
    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);

    /* backfill & link */
    val.len = 0;
    item_backfill(it, &val);
    item_insert(it, &key);
    test_assert_reserve_backfill_link_exists(it);

    test_reset(0);

    test_assert_reserve_backfill_link_exists(it);

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
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);
    append = str2bstr(APPEND);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &append, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    test_reset(0);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(!it->is_raligned, "item with key %.*s is raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + append.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
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
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);
    prepend = str2bstr(PREPEND);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &prepend, false);
    ck_assert_msg(status == ITEM_OK, "item_prepend not OK - return status %d", status);

    test_reset(0);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(it->is_linked, "item with key %.*s not linked", key.len, key.data);
    ck_assert_msg(!it->in_freeq, "linked item with key %.*s in freeq", key.len, key.data);
    ck_assert_msg(it->is_raligned, "item with key %.*s is not raligned", key.len, key.data);
    ck_assert_int_eq(it->vlen, val.len + prepend.len);
    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(item_data(it), PREPEND VAL, val.len + prepend.len), 0);
#undef KEY
#undef VAL
#undef PREPEND
}
END_TEST

START_TEST(test_annex_sequence)
{
#define KEY "key"
#define VAL "val"
#define PREPEND "prepend"
#define APPEND1 "append1"
#define APPEND2 "append2"
    struct bstring key, val, prepend, append1, append2;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    prepend = str2bstr(PREPEND);
    append1 = str2bstr(APPEND1);
    append2 = str2bstr(APPEND2);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    status = item_annex(it, &key, &append1, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    test_assert_annex_sequence_exists(key, val.len + append1.len, VAL APPEND1, true);
    test_reset(0);
    test_assert_annex_sequence_exists(key, val.len + append1.len, VAL APPEND1, true);

    it = item_get(&key);
    status = item_annex(it, &key, &prepend, false);
    ck_assert_msg(status == ITEM_OK, "item_prepend not OK - return status %d", status);

    test_assert_annex_sequence_exists(key, val.len + append1.len + prepend.len, PREPEND VAL APPEND1, false);
    test_reset(0);
    test_assert_annex_sequence_exists(key, val.len + append1.len + prepend.len, PREPEND VAL APPEND1, false);

    it = item_get(&key);
    status = item_annex(it, &key, &append2, true);
    ck_assert_msg(status == ITEM_OK, "item_append not OK - return status %d", status);

    test_assert_annex_sequence_exists(key,  val.len + append1.len + prepend.len + append2.len, PREPEND VAL APPEND1 APPEND2, true);

    test_reset(0);

    test_assert_annex_sequence_exists(key,  val.len + append1.len + prepend.len + append2.len, PREPEND VAL APPEND1 APPEND2, true);

#undef KEY
#undef VAL
#undef PREPEND
#undef APPEND1
#undef APPEND2
}
END_TEST

START_TEST(test_update_basic)
{
#define KEY "key"
#define OLD_VAL "old_val"
#define NEW_VAL "new_val"
    struct bstring key, old_val, new_val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

    time_update();
    status = item_reserve(&it, &key, &old_val, old_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    item_update(it, &new_val);

    test_assert_update_basic_entry_exists(key);

    test_reset(0);

    test_assert_update_basic_entry_exists(key);

#undef KEY
#undef OLD_VAL
#undef NEW_VAL
}
END_TEST

START_TEST(test_delete_basic)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    ck_assert_msg(item_delete(&key), "item_delete for key %.*s not successful", key.len, key.data);

    it = item_get(&key);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after delete", key.len, key.data);

    test_reset(0);

    it = item_get(&key);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after delete", key.len, key.data);

#undef KEY
#undef VAL
}
END_TEST

START_TEST(test_flush_basic)
{
#define KEY1 "key1"
#define VAL1 "val1"
#define KEY2 "key2"
#define VAL2 "val2"
    struct bstring key1, val1, key2, val2;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key1 = str2bstr(KEY1);
    val1 = str2bstr(VAL1);

    key2 = str2bstr(KEY2);
    val2 = str2bstr(VAL2);

    time_update();
    status = item_reserve(&it, &key1, &val1, val1.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key1);

    time_update();
    status = item_reserve(&it, &key2, &val2, val2.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key2);

    item_flush();

    it = item_get(&key1);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush", key1.len, key1.data);
    it = item_get(&key2);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush", key2.len, key2.data);

    test_reset(0);

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

START_TEST(test_update_basic_after_restart)
{
#define KEY "key"
#define OLD_VAL "old_val"
#define NEW_VAL "new_val"
    struct bstring key, old_val, new_val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

    time_update();
    status = item_reserve(&it, &key, &old_val, old_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);

    test_reset(0);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key %.*s", key.len, key.data);
    item_update(it, &new_val);
    test_assert_update_basic_entry_exists(key);

#undef KEY
#undef OLD_VAL
#undef NEW_VAL
}
END_TEST

START_TEST(test_expire_basic)
{
#define KEY "key"
#define VAL "val"
#define TIME 12345678
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    proc_sec = TIME;
    status = item_reserve(&it, &key, &val, val.len, 0, TIME + 1);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    test_reset(0);

    test_assert_expire_exists(key, 2);

#undef KEY
#undef VAL
#undef TIME
}
END_TEST

START_TEST(test_expire_truncated)
{
#define KEY "key"
#define VAL "value"
#define TIME 12345678
#define TTL_MAX 10
#define TTL_LONG (TTL_MAX + 5)
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset(1);
    max_ttl = TTL_MAX;

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    proc_sec = TIME;
    status = item_reserve(&it, &key, &val, val.len, 0, TIME + TTL_LONG);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    item_insert(it, &key);

    test_reset(0);

    test_assert_expire_exists(key, (TTL_MAX + 2));

#undef KEY
#undef VAL
#undef TIME
#undef TTL_MAX
#undef TTL_LONG
}
END_TEST

/**
 * Tests check lruq state after restart
 */
START_TEST(test_lruq_rebuild)
{
#define NUM_ITEMS 3
#define KEY1 "key1"
#define VLEN1 5
#define KEY2 "key2"
#define VLEN2 (1 * KiB)
#define KEY3 "key3"
#define VLEN3 (1000 * KiB)

    struct bstring key[NUM_ITEMS] = {
        str2bstr(KEY1),
        str2bstr(KEY2),
        str2bstr(KEY3),
    };
    struct bstring val[NUM_ITEMS];
    struct item *it[NUM_ITEMS];
    struct slab *slab[NUM_ITEMS+1] = { NULL };
    item_rstatus_e status;

    test_reset(1);

    val[0].data = cc_alloc(VLEN1);
    cc_memset(val[0].data, 'A', VLEN1);
    val[0].len = VLEN1;

    val[1].data = cc_alloc(VLEN2);
    cc_memset(val[1].data, 'B', VLEN2);
    val[1].len = VLEN2;

    val[2].data = cc_alloc(VLEN3);
    cc_memset(val[2].data, 'C', VLEN3);
    val[2].len = VLEN3;

    time_update();

    for (int i = 0; i < NUM_ITEMS; ++i) {
        status = item_reserve(&it[i], &key[i], &val[i], val[i].len, 0, INT32_MAX);
        free(val[i].data);
        ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
        item_insert(it[i], &key[i]);
    }

    for (int i = 0; i < NUM_ITEMS; ++i) {
        struct item *it_temp  = item_get(&key[i]);
        ck_assert_msg(it_temp != NULL, "item_get could not find key %.*s", key[i].len, key[i].data);
        slab[i] = item_to_slab(it_temp);
    }

    for (int i = 0; i < NUM_ITEMS; ++i) {
        ck_assert_ptr_eq(TAILQ_NEXT(slab[i], s_tqe), slab[i+1]);
        ck_assert_ptr_eq(*(slab[i]->s_tqe.tqe_prev), slab[i]);
    }

    test_reset(0);

    for (int i = 0; i < NUM_ITEMS; ++i) {
        struct item *it_temp  = item_get(&key[i]);
        ck_assert_msg(it_temp != NULL, "item_get could not find key %.*s", key[i].len, key[i].data);
        slab[i] = item_to_slab(it_temp);
    }

    for (int i = 0; i < NUM_ITEMS; ++i) {
        ck_assert_ptr_eq(TAILQ_NEXT(slab[i], s_tqe), slab[i+1]);
        ck_assert_ptr_eq(*(slab[i]->s_tqe.tqe_prev), slab[i]);
    }

#undef NUM_ITEMS
#undef KEY1
#undef VLEN1
#undef KEY2
#undef VLEN2
#undef KEY3
#undef VLEN3
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
    item_rstatus_e status;
    struct item *it;

    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_size.val.vuint = MY_SLAB_SIZE;
    options.slab_mem.val.vuint = MY_SLAB_MAXBYTES;
    options.slab_evict_opt.val.vuint = EVICT_CS;
    options.slab_item_max.val.vuint = MY_SLAB_SIZE - SLAB_HDR_SIZE;
    options.slab_datapool.val.vstr = DATAPOOL_PATH;

    test_teardown(1);
    slab_setup(&options, &metrics);

    for (i = 0; i < NUM_ITEMS + 1; i++) {
        time_update();
        status = item_reserve(&it, &key[i], &val[i], val[i].len, 0, INT32_MAX);
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

    test_teardown(0);
    slab_setup(&options, &metrics);

    ck_assert_msg(item_get(&key[0]) == NULL,
        "item 0 found afer restart, expected to be evicted");
    ck_assert_msg(item_get(&key[1]) == NULL,
        "item 1 found after restart, expected to be evicted");
    ck_assert_msg(item_get(&key[2]) != NULL,
        "item 2 not found after restart");


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
    item_rstatus_e status;
    struct item *it;
    struct slab * s;

    test_reset(1);

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    /* reserve & release */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
    s = item_to_slab(it);

    test_reset(0);

    ck_assert_msg(s->refcount == 0, "slab refcount %"PRIu32"; 0 expected", s->refcount);

    /* reserve & backfill (& link) */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
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
#define KEY "key"
#define VAL "val"
    /**
     * The slab will be created with these parameters:
     *   slab size 96, slab hdr size 36, item hdr size 40
     * Given that cas 8,
     * we know: key + val < 12
     *
     **/
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *nit;

    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_size.val.vuint = MY_SLAB_SIZE;
    options.slab_mem.val.vuint = MY_SLAB_MAXBYTES;
    options.slab_evict_opt.val.vuint = EVICT_CS;
    options.slab_item_max.val.vuint = MY_SLAB_SIZE - SLAB_HDR_SIZE;
    options.slab_datapool.val.vstr = DATAPOOL_PATH;

    test_teardown(1);
    slab_setup(&options, &metrics);
    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);

    status = item_reserve(&nit, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_ENOMEM, "item_reserve should fail - return status %d", status);

    item_insert(it, &key); /* clears slab refcount, can be evicted */

    test_teardown(0);
    slab_setup(&options, &metrics);

    status = item_reserve(&nit, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d", status);
#undef KEY
#undef VAL
#undef MY_SLAB_SIZE
#undef MY_SLAB_MAXBYTES
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
    tcase_add_test(tc_item, test_update_basic_after_restart);
    tcase_add_test(tc_item, test_expire_basic);
    tcase_add_test(tc_item, test_expire_truncated);

    TCase *tc_slab = tcase_create("slab api");
    suite_add_tcase(s, tc_slab);
    tcase_add_test(tc_slab, test_lruq_rebuild);
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
    test_teardown(1);

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
