#include <storage/seg/item.h>
#include <storage/seg/seg.h>
#include <storage/seg/ttlbucket.h>

#include <time/time.h>

#include <cc_bstring.h>
#include <cc_mm.h>

#include <check.h>
#include <stdio.h>
#include <string.h>

/* define for each suite, local scope due to macro visibility rule */
#define SUITE_NAME "seg"
#define DEBUG_LOG SUITE_NAME ".log"

seg_options_st options = {SEG_OPTION(OPTION_INIT)};
seg_metrics_st metrics = {SEG_METRIC(METRIC_INIT)};


/*
 * utilities
 */
static void
test_setup(void)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    seg_setup(&options, &metrics);
}

static void
test_teardown(void)
{
    seg_teardown();
}

static void
test_reset(void)
{
    test_teardown();
    test_setup();
}

/**
 * Tests basic functionality for item_insert with small key/val. Checks that the
 * commands succeed and that the item returned is well-formed.
 */


START_TEST(test_item_basic)
{
    ck_assert_int_eq(item_size_roundup(1), 8);
    ck_assert_int_eq(item_size_roundup(2), 8);
    ck_assert_int_eq(item_size_roundup(4), 8);
    ck_assert_int_eq(item_size_roundup(7), 8);
    ck_assert_int_eq(item_size_roundup(8), 8);
    ck_assert_int_eq(item_size_roundup(101), 104);
}

START_TEST(test_ttl_bucket_find)
{
    test_reset();

    delta_time_i ttl;
    uint32_t idx_true, idx;

    ttl = 7;
    idx_true = 0;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);
    ttl = 8;
    idx_true = 1;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);
    ttl = 200;
    idx_true = 25;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);
    ttl = 2000;
    idx_true = 250;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);

    ttl = 20000;
    idx_true = 412;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);

    ttl = 200000;
    idx_true = 609;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);

    ttl = 2000000;
    idx_true = 829;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);

    ttl = MAX_TTL - 1;
    idx_true = MAX_TTL_BUCKET_IDX;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);

    ttl = INT32_MAX;
    idx_true = MAX_TTL_BUCKET_IDX;
    idx = find_ttl_bucket_idx(ttl);
    ck_assert_msg(
            idx == idx_true, "ttl %u bucket idx %u != %u", ttl, idx_true, idx);
}
END_TEST


START_TEST(test_insert_basic)
{
#define KEY "key"
#define VAL "val"
#define MLEN 8
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *it2;

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, MLEN, INT32_MAX);
    printf("status %d\n", status);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    ck_assert_msg(it != NULL, "item_reserve with key %.*s reserved NULL item",
            key.len, key.data);

    ck_assert_msg(!it->seg_id, "item with key %.*s not linked", 0);
    ck_assert_msg(!it->is_num, "linked item with key %.*s in freeq", key.len,
            key.data);

    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(item_nkey(it), sizeof(KEY) - 1);
    ck_assert_int_eq(it->vlen, sizeof(VAL) - 1);
    ck_assert_int_eq(item_nval(it), MAX(sizeof(VAL) - 1, sizeof(uint64_t)));
    ck_assert_int_eq(item_olen(it), MLEN);

    ck_assert_int_eq(item_val(it) - (char *)it,
            offsetof(struct item, end) + +MLEN + sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(item_val(it), VAL, val.len), 0);

    item_insert(it);
    it2 = item_get(&key);
    ck_assert_msg(
            it2 != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(
            it2 == it, "item_get returns a different item %p %p", it2, it);
    item_release(it2);

#undef MLEN
#undef KEY
#undef VAL
}
END_TEST

/**
 * Tests item_insert and item_get for large value (close to 1 MiB). Checks that
 * the commands succeed and that the item returned is well-formed.
 */
START_TEST(test_insert_large)
{
#define KEY "key"
#define VLEN (1000 * KiB)

    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *it2;
    size_t len;
    char *p;

    test_reset();

    key = str2bstr(KEY);

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    cc_free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    it2 = item_get(&key);
    ck_assert_msg(
            it2 != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(
            it2 == it, "item_get returns a different item %p %p", it2, it);
    ck_assert_int_eq(it2->vlen, VLEN);
    ck_assert_int_eq(it2->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(KEY, item_key(it), sizeof(KEY) - 1), 0);

    for (p = item_val(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--)
        ;
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", VLEN,
            item_val(it));
    item_release(it2);

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
    char *p;

    test_reset();

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


    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->vlen, val.len);
    for (p = item_val(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--)
        ;
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", it->vlen,
            item_val(it));

    /* backfill */
    val.len = vlen - val.len;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'B', val.len);
    item_backfill(it, &val);
    free(val.data);
    ck_assert_int_eq(it->vlen, vlen);
    for (p = item_val(it) + vlen - val.len, len = val.len; len > 0 && *p == 'B';
            p++, len--)
        ;
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", val.len,
            item_nval(it) + vlen - val.len);

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
    size_t len;
    char *p;

    test_reset();

    key = str2bstr(KEY);

    val.len = VLEN;
    val.data = cc_alloc(val.len);
    cc_memset(val.data, 'A', val.len);

    /* reserve */
    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);

    /* backfill & link */
    val.len = 0;
    item_backfill(it, &val);
    item_insert(it);
    ck_assert_int_eq(it->vlen, VLEN);

    for (p = item_val(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--)
        ;
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", VLEN,
            item_val(it));

#undef VLEN
#undef KEY
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
    item_rstatus_e status;
    struct item *oit, *nit;

    test_reset();

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

    time_update();
    status = item_reserve(&oit, &key, &old_val, old_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(oit);

    oit = item_get(&key);
    ck_assert_msg(
            oit != NULL, "item_get could not find key %.*s", key.len, key.data);
    item_release(oit);

    status = item_reserve(&nit, &key, &new_val, new_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_update(nit);

    nit = item_get(&key);
    ck_assert_msg(
            nit != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_int_eq(nit->vlen, new_val.len);
    ck_assert_int_eq(nit->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(item_val(nit), NEW_VAL, new_val.len), 0);

#undef KEY
#undef OLD_VAL
#undef NEW_VAL
}
END_TEST


/* test insert_or_update_func */
START_TEST(test_insert_or_update_basic)
{
#define KEY "key"
#define OLD_VAL "old_val"
#define NEW_VAL "new_val"
    struct bstring key, old_val, new_val;
    item_rstatus_e status;
    struct item *oit, *nit;

    test_reset();

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

    /* insert */
    status = item_reserve(&oit, &key, &old_val, old_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert_or_update(oit);

    oit = item_get(&key);
    ck_assert_msg(
            oit != NULL, "item_get could not find key %.*s", key.len, key.data);

    ck_assert_int_eq(oit->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(oit->vlen, sizeof(OLD_VAL) - 1);
    ck_assert_int_eq(cc_memcmp(item_val(oit), OLD_VAL, old_val.len), 0);
    item_release(oit);

    /* update */
    status = item_reserve(&nit, &key, &new_val, new_val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert_or_update(nit);

    nit = item_get(&key);
    ck_assert_msg(
            nit != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_int_eq(nit->vlen, new_val.len);
    ck_assert_int_eq(nit->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(cc_memcmp(item_val(nit), NEW_VAL, new_val.len), 0);

    ck_assert_int_eq(oit->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(oit->vlen, sizeof(OLD_VAL) - 1);
    ck_assert_int_eq(cc_memcmp(item_val(oit), OLD_VAL, old_val.len), 0);
    item_release(nit);


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
    item_rstatus_e status;
    struct item *it;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    it = item_get(&key);
    ck_assert_msg(
            it != NULL, "item_get could not find key %.*s", key.len, key.data);
    item_release(it);

    ck_assert_msg(item_delete(&key), "item_delete for key %.*s not successful",
            key.len, key.data);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after delete",
            key.len, key.data);

#undef KEY
#undef VAL
}
END_TEST


START_TEST(test_delete_more)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    bool in_cache;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();

    /* test deleting items not in the hashtable */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key");
    item_release(it);

    in_cache = item_delete(&key);
    it = item_get(&key);
    ck_assert_msg(in_cache, "item_delete return False on successful deletion");
    ck_assert_msg(it == NULL, "item still exists after delete");
    in_cache = item_delete(&key);
    ck_assert_msg(in_cache == false, "delete the same item twice return true");

    in_cache = item_delete(&val);
    ck_assert_msg(in_cache == false, "delete item never inserted return true");

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
    item_rstatus_e status;
    struct item *it;

    test_reset();

    key1 = str2bstr(KEY1);
    val1 = str2bstr(VAL1);

    key2 = str2bstr(KEY2);
    val2 = str2bstr(VAL2);

    time_update();
    status = item_reserve(&it, &key1, &val1, val1.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    time_update();
    status = item_reserve(&it, &key2, &val2, val2.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    item_flush();
    it = item_get(&key1);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush",
            key1.len, key1.data);

    it = item_get(&key2);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after flush",
            key2.len, key2.data);

#undef KEY1
#undef VAL1
#undef KEY2
#undef VAL2
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

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    proc_sec = TIME;
    status = item_reserve(&it, &key, &val, val.len, 0, TIME + 1);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get on unexpired item not successful");
    ck_assert_msg(item_to_seg(it)->refcount == 1, "seg refcount incorrect");

    item_release(it);
    ck_assert_msg(item_to_seg(it)->refcount == 0, "seg refcount incorrect");

    proc_sec += 2;
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item_get returned not NULL after expiration");

#undef KEY
#undef VAL
#undef TIME
}
END_TEST


START_TEST(test_item_numeric)
{
#define KEY "key"
#define VAL "1"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, 0, 0);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - status %d", status);
    item_insert(it);

    uint64_t vint;
    status = item_incr(&vint, it, 0);
    ck_assert_int_eq(vint, atoi(VAL));
    ck_assert_msg(status == ITEM_OK, "item_incr not OK - status %d", status);

    item_incr(&vint, it, 28);
    ck_assert_int_eq(vint, atoi(VAL) + 28);
    ck_assert_msg(status == ITEM_OK, "item_incr not OK - status %d", status);

    item_incr(&vint, it, 24);
    ck_assert_int_eq(vint, atoi(VAL) + 52);

#undef KEY
#undef VAL
}
END_TEST


START_TEST(test_seg_basic)
{
#define KEY "key"
#define VLEN (1000 * KiB)
#define TIME 12345678

    struct bstring key, val;

    test_reset();

    key = str2bstr(KEY);

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    test_reset();

    struct seg *seg;
    for (uint32_t i = 0; i < 63; i++) {
        seg = seg_get_new();
        ck_assert_int_eq(seg->seg_id, i);
        ck_assert_int_eq(seg->initialized, 1);
    }

#undef KEY
#undef VAL
#undef TIME
}
END_TEST

START_TEST(test_seg_more)
{
#define KEY "key"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    bool in_cache;
    struct seg *seg;
    uint32_t offset, occu_size, n_item;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    time_update();

    /* test deleting items not in the hashtable */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    seg = item_to_seg(it);
    offset = seg->write_offset;
    occu_size = seg->occupied_size;
    n_item = seg->n_item;

    ck_assert_int_eq(seg->locked, 0);
    ck_assert_int_eq(seg->refcount, 0);
    ck_assert_int_eq(seg->sealed, 0);

                   it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key");
    item_release(it);

    in_cache = item_delete(&key);
    it = item_get(&key);
    ck_assert_msg(in_cache, "item_delete return False on successful deletion");
    ck_assert_msg(it == NULL, "item still exists after delete");
    in_cache = item_delete(&key);
    ck_assert_msg(in_cache == false, "delete the same item twice return true");

    in_cache = item_delete(&val);
    ck_assert_msg(in_cache == false, "delete item never inserted return true");

}
END_TEST


START_TEST(test_ttl_bucket_basic)
{
#define KEY "key"
#define VLEN (1000 * KiB)
#define TIME 12345678

    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *it2;
    uint32_t offset, occu_size;

    test_reset();

    key = str2bstr(KEY);

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    test_reset();

    struct seg *seg1, *seg2;
    for (uint32_t i = 0; i < 4; i++) {
        status = item_reserve(
                &it, &key, &val, val.len, 0, time_proc_sec() + 8 * i + 2);
        ck_assert_msg(status == ITEM_OK,
                "item_reserve not OK - return status %d", status);
        seg1 = item_to_seg(it);
        ck_assert_msg(TAILQ_FIRST(&ttl_buckets[i].seg_q) == seg1,
                "ttl_bucket queue not correct %p != %p",
                TAILQ_FIRST(&ttl_buckets[i].seg_q), seg1);
        ck_assert_int_eq(seg1->seg_id, i * 2);
        offset = seg1->write_offset;
        occu_size = seg1->occupied_size;
        ck_assert_msg(offset == item_ntotal(it) ||
                        offset == item_ntotal(it) + sizeof(uint64_t),
                "seg write offset is incorrect %d", offset);
        ck_assert_msg(occu_size == item_ntotal(it) ||
                        occu_size == item_ntotal(it) + sizeof(uint64_t),
                "seg occupied size is incorrect %", occu_size);
        item_insert_or_update(it);

        /* insert another item of the same key, val and ttl,
         * which should occupy another seg in the ttl bucket and
         * replace the previous item in the hash table */
        status = item_reserve(
                &it, &key, &val, val.len, 0, time_proc_sec() + 8 * i + 2);
        ck_assert_msg(status == ITEM_OK,
                "item_reserve not OK - return status %d", status);
        seg2 = item_to_seg(it);
        ck_assert_msg(TAILQ_FIRST(&ttl_buckets[i].seg_q) == seg1,
                "ttl_bucket queue head not correct");
        ck_assert_msg(TAILQ_LAST(&ttl_buckets[i].seg_q, seg_tqh) == seg2,
                "ttl_bucket queue tail not correct");
        ck_assert_int_eq(seg1->seg_id, i * 2);
        ck_assert_int_eq(seg2->seg_id, i * 2 + 1);

        offset = seg2->write_offset;
        occu_size = seg2->occupied_size;
        ck_assert_msg(offset == item_ntotal(it) ||
                        offset == item_ntotal(it) + sizeof(uint64_t),
                "seg write offset is incorrect %d", offset);
        ck_assert_msg(occu_size == item_ntotal(it) ||
                        occu_size == item_ntotal(it) + sizeof(uint64_t),
                "seg occupied size is incorrect %", occu_size);

        item_insert_or_update(it);

        offset = seg1->write_offset;
        occu_size = seg1->occupied_size;
        ck_assert_msg(offset == item_ntotal(it) ||
                        offset == item_ntotal(it) + sizeof(uint64_t),
                "seg write offset is incorrect %d", offset);
        ck_assert_msg(occu_size == 0 || occu_size == sizeof(uint64_t),
                "seg occupied size is incorrect %", occu_size);

        it2 = item_get(&key);
        ck_assert_msg(it2 == it, "update item is incorrect");
        item_release(it2);
    }

#undef KEY
#undef VAL
#undef TIME
}
END_TEST


#ifdef do_not_define
START_TEST(test_evict_lru_basic)
{
#    define MY_seg_SIZE 160
#    define MY_seg_MAXBYTES 160
    /**
     * These are the segs that will be created with these parameters:
     *
     * seg size 160, seg hdr size 36, item hdr size 40, item chunk size44, total
     *memory 320 class   1: items       2  size      48  data       8  slack 28
     * class   2: items       1  size     120  data      80  slack       4
     *
     * If we use 8 bytes of key+value, it will use the class 1 that can fit
     * two elements. The third one will cause a full seg eviction.
     *
     **/
#    define KEY_LENGTH 2
#    define VALUE_LENGTH 8
#    define NUM_ITEMS 2

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
    options.seg_size.val.vuint = MY_seg_SIZE;
    options.seg_mem.val.vuint = MY_seg_MAXBYTES;
    options.seg_evict_opt.val.vuint = EVICT_CS;
    options.seg_item_max.val.vuint = MY_seg_SIZE - seg_HDR_SIZE;

    test_teardown();
    seg_setup(&options, &metrics);

    for (i = 0; i < NUM_ITEMS + 1; i++) {
        time_update();
        status = item_reserve(&it, &key[i], &val[i], val[i].len, 0, INT32_MAX);
        ck_assert_msg(status == ITEM_OK,
                "item_reserve not OK - return status %d", status);
        item_insert(it, &key[i]);
        ck_assert_msg(item_get(&key[i]) != NULL, "item %lu not found", i);
    }

    ck_assert_msg(
            item_get(&key[0]) == NULL, "item 0 found, expected to be evicted");
    ck_assert_msg(
            item_get(&key[1]) == NULL, "item 1 found, expected to be evicted");
    ck_assert_msg(item_get(&key[2]) != NULL, "item 2 not found");

#    undef KEY_LENGTH
#    undef VALUE_LENGTH
#    undef NUM_ITEMS
#    undef MY_seg_SIZE
#    undef MY_seg_MAXBYTES
}
END_TEST

START_TEST(test_refcount)
{
#    define KEY "key"
#    define VAL "val"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    struct seg *s;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    /* reserve & release */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    s = item_to_seg(it);
    ck_assert_msg(s->refcount == 1, "seg refcount %" PRIu32 "; 1 expected",
            s->refcount);
    item_release(&it);
    ck_assert_msg(s->refcount == 0, "seg refcount %" PRIu32 "; 0 expected",
            s->refcount);

    /* reserve & backfill (& link) */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    s = item_to_seg(it);
    ck_assert_msg(s->refcount == 1, "seg refcount %" PRIu32 "; 1 expected",
            s->refcount);
    val = null_bstring;
    item_backfill(it, &val);
    item_insert(it, &key);
    ck_assert_msg(s->refcount == 0, "seg refcount %" PRIu32 "; 0 expected",
            s->refcount);
}
END_TEST

START_TEST(test_evict_refcount)
{
#    define MY_seg_SIZE 96
#    define MY_seg_MAXBYTES 96
#    define KEY "key"
#    define VAL "val"
    /**
     * The seg will be created with these parameters:
     *   seg size 96, seg hdr size 36, item hdr size 40
     * Given that cas 8,
     * we know: key + val < 12
     *
     **/
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *nit;

    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.seg_size.val.vuint = MY_seg_SIZE;
    options.seg_mem.val.vuint = MY_seg_MAXBYTES;
    options.seg_evict_opt.val.vuint = EVICT_CS;
    options.seg_item_max.val.vuint = MY_seg_SIZE - seg_HDR_SIZE;

    test_teardown();
    seg_setup(&options, &metrics);
    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    status = item_reserve(&nit, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_ENOMEM,
            "item_reserve should fail - return status %d", status);

    item_insert(it, &key); /* clears seg refcount, can be evicted */
    status = item_reserve(&nit, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
#    undef KEY
#    undef VAL
#    undef MY_seg_SIZE
#    undef MY_seg_MAXBYTES
}
END_TEST
#endif

/*
 * test suite
 */
static Suite *
seg_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

    /* basic item */
    TCase *tc_item = tcase_create("item api");
    suite_add_tcase(s, tc_item);
    tcase_add_test(tc_item, test_item_basic);
    tcase_add_test(tc_item, test_insert_basic);
    tcase_add_test(tc_item, test_insert_large);
    tcase_add_test(tc_item, test_insert_or_update_basic);
    tcase_add_test(tc_item, test_update_basic);
    tcase_add_test(tc_item, test_reserve_backfill_release);
    tcase_add_test(tc_item, test_reserve_backfill_link);
    tcase_add_test(tc_item, test_delete_basic);
    tcase_add_test(tc_item, test_delete_more);
    tcase_add_test(tc_item, test_flush_basic);
    tcase_add_test(tc_item, test_expire_basic);
    tcase_add_test(tc_item, test_item_numeric);

    TCase *tc_ttl = tcase_create("ttl_bucket api");
    suite_add_tcase(s, tc_ttl);
    tcase_add_test(tc_item, test_ttl_bucket_find);
    tcase_add_test(tc_item, test_ttl_bucket_basic);


    TCase *tc_seg = tcase_create("seg api");
    suite_add_tcase(s, tc_seg);
    tcase_add_test(tc_seg, test_seg_basic);
    tcase_add_test(tc_seg, test_seg_more);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    Suite *suite = seg_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    //    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
