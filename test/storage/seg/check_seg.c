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
    //    time_update();
    proc_sec = 0;
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
#define KEY "test_insert_basic"
#define VAL "val"
#define MLEN 8
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *it2;

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, MLEN, INT32_MAX);
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
    ck_assert_int_eq(cc_memcmp(item_key(it), KEY, key.len), 0);
    ck_assert_int_eq(cc_memcmp(item_val(it), VAL, val.len), 0);
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 1, "seg refcount incorrect");

    item_insert(it);
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");

    it2 = item_get(&key);
    ck_assert_msg(
            it2 != NULL, "item_get could not find key %.*s", key.len, key.data);
    ck_assert_msg(
            it2 == it, "item_get returns a different item %p %p", it2, it);
    ck_assert_msg(item_to_seg(it)->r_refcount == 1, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");
    item_release(it2);
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");

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
#define KEY "test_insert_large"
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
#define KEY "test_reserve_backfill_release"
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
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK status %d", status);
    ck_assert_msg(it != NULL, "item_reserve returned NULL object");

    ck_assert_int_eq(it->klen, sizeof(KEY) - 1);
    ck_assert_int_eq(it->vlen, val.len);
    for (p = item_val(it), len = it->vlen; len > 0 && *p == 'A'; p++, len--)
        ;
    ck_assert_msg(len == 0, "item_data contains wrong value %.*s", it->vlen,
            item_val(it));

    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 1, "seg refcount incorrect");

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
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 1, "seg refcount incorrect");

#undef VLEN
#undef KEY
}
END_TEST

START_TEST(test_reserve_backfill_link)
{
#define KEY "test_reserve_backfill_link"
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
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    free(val.data);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);

    /* backfill & link */
    val.len = 0;
    item_backfill(it, &val);
    item_insert(it);
    ck_assert_int_eq(it->vlen, VLEN);
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");

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
#define KEY "test_update_basic"
#define OLD_VAL "old_val"
#define NEW_VAL "new_val"
    struct bstring key, old_val, new_val;
    item_rstatus_e status;
    struct item *oit, *nit;

    test_reset();

    key = str2bstr(KEY);
    old_val = str2bstr(OLD_VAL);
    new_val = str2bstr(NEW_VAL);

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
#define KEY "test_insert_or_update_basic"
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
#define KEY "test_delete_basic"
#define VAL "valvalvalvalvalvalvalvalval"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    struct seg *seg;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);
    seg = item_to_seg(it);

    it = item_get(&key);
    ck_assert_msg(
            it != NULL, "item_get could not find key %.*s", key.len, key.data);
    item_release(it);

    ck_assert_msg(item_delete(&key), "item_delete for key %.*s not successful",
            key.len, key.data);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item with key %.*s still exists after delete",
            key.len, key.data);
    ck_assert(seg->n_item == 0);
    ck_assert(seg->write_offset >= cc_strlen(VAL));
    ck_assert(seg->occupied_size <= sizeof(uint64_t));
    ck_assert(seg->r_refcount == 0);
    ck_assert(seg->w_refcount == 0);

#undef KEY
#undef VAL
}
END_TEST


START_TEST(test_delete_more)
{
#define KEY "test_delete_more"
#define VAL "val"
    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    struct seg *seg;
    bool in_cache;

    test_reset();

    key = str2bstr(KEY);
    val = str2bstr(VAL);

    /* test deleting items not in the hashtable */
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK status %d", status);
    item_insert(it);

    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key");
    item_release(it);

    seg = item_to_seg(it);
    ck_assert_int_eq(seg->seg_id, 0);
    ck_assert_int_eq(seg->locked, 0);
    ck_assert_msg(seg->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
    ck_assert_int_eq(seg->n_item, 1);
    ck_assert_int_eq(seg->write_offset, seg->occupied_size);

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
#define KEY1 "test_flush_basic1"
#define VAL1 "val1"
#define KEY2 "test_flush_basic2"
#define VAL2 "val2"
    struct bstring key1, val1, key2, val2;
    item_rstatus_e status;
    struct item *it;

    test_reset();

    key1 = str2bstr(KEY1);
    val1 = str2bstr(VAL1);

    key2 = str2bstr(KEY2);
    val2 = str2bstr(VAL2);

    status = item_reserve(&it, &key1, &val1, val1.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

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
#define KEY "test_expire_basic"
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
    ck_assert_msg(item_to_seg(it)->r_refcount == 1, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");

    item_release(it);
    ck_assert_msg(item_to_seg(it)->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(item_to_seg(it)->w_refcount == 0, "seg refcount incorrect");

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
#define KEY "test_item_numeric"
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
    test_reset();

    int32_t seg_id;
    for (uint32_t i = 0; i < 8; i++) {
        seg_id = seg_get_new();
        ck_assert_int_eq(seg_id, i);
        ck_assert_int_eq(heap.segs[seg_id].initialized, 1);
    }
}
END_TEST

START_TEST(test_seg_more)
{
//#define KEY "test_seg_more"
#define VLEN (1000 * KiB)
#define MEM_SIZE "4194304"

    seg_teardown();
    option_set(&options.seg_mem, MEM_SIZE);
    seg_setup(&options, &metrics);

    char *keys[] = {"seg-0", "seg-1", "seg-2", "seg-3", "seg-4", "seg-5",
            "seg-6", "seg-7", "seg-8"};

    struct bstring key, val;
    item_rstatus_e status;
    struct item *it;
    struct seg *seg;

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    for (int i = 0; i < 4; i++) {
        bstring_set_cstr(&key, keys[i]);
        status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
        ck_assert_msg(status == ITEM_OK, "item_reserve not OK %d", status);
        item_insert(it);
        it = item_get(&key);
        ck_assert_msg(it != NULL, "item_get could not find key");
        item_release(it);

        seg = item_to_seg(it);
        ck_assert_int_eq(seg->seg_id, i);
        ck_assert_int_eq(seg->locked, 0);
        ck_assert_msg(seg->r_refcount == 0, "seg refcount incorrect");
        ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
        ck_assert_int_eq(seg->n_item, 1);
        ck_assert_int_eq(seg->write_offset, seg->occupied_size);
        ck_assert_int_eq(seg->prev_seg_id, i - 1);
        if (i > 0)
            ck_assert_int_eq(heap.segs[i - 1].next_seg_id, i);
    }

    /* remove all item of seg 2 and return to global pool */
    seg_rm_all_item(2);
    seg_return_seg(2);

    ck_assert_msg(heap.free_seg_id == 2);
    heap.segs[2].prev_seg_id = -1;
    heap.segs[2].next_seg_id = -1;

    bstring_set_cstr(&key, keys[4]);
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK status %d", status);
    item_insert(it);
    it = item_get(&key);
    ck_assert_msg(it != NULL, "item_get could not find key");
    item_release(it);

    seg = item_to_seg(it);
    ck_assert_int_eq(seg->seg_id, 2);
    ck_assert_int_eq(seg->locked, 0);
    ck_assert_msg(seg->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
    ck_assert_int_eq(seg->n_item, 1);
    ck_assert_int_eq(seg->write_offset, seg->occupied_size);

#undef KEY
#undef VAL
}
END_TEST

START_TEST(test_segevict_FIFO)
{
#define KEY "test_segevict_FIFO"
#define VLEN (1000 * KiB)
#define MEM_SIZE "4194304"

    char *keys[] = {"fifo-0", "fifo-1", "fifo-2", "fifo-3", "fifo-4", "fifo-5",
            "fifo-6", "fifo-7", "fifo-8"};
    seg_teardown();
    option_set(&options.seg_mem, MEM_SIZE);
    option_set(&options.seg_evict_opt, "2");
    seg_setup(&options, &metrics);

    struct bstring key, val;
    struct item *it;
    struct seg *seg;
    item_rstatus_e status;

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    ck_assert_msg(
            heap.max_nseg == 4, "max_nseg incorrect %" PRIu32, heap.max_nseg);

    for (uint32_t i = 0; i < 4; i++) {
        proc_sec++;
        bstring_set_literal(&key, keys[i]);
        status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
        ck_assert(it != NULL);
        item_insert(it);
        it = item_get(&key);
        ck_assert(it != NULL);
        item_release(it);
    }

    /* cache is full at this time, check before next insert (and evict) */
    bstring_set_literal(&key, keys[0]);
    it = item_get(&key);
    ck_assert(it != NULL);
    ck_assert_int_eq(it->seg_id, 0);
    seg = item_to_seg(it);
    ck_assert_msg(seg->r_refcount == 1, "seg refcount incorrect");
    ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->write_offset == item_ntotal(it) ||
                    seg->write_offset == item_ntotal(it) + 8,
            "write offset error %" PRIu32, seg->write_offset);
    ck_assert_msg(seg->write_offset == seg->occupied_size);
    ck_assert(seg->n_item == 1);
    item_release(it);

    /* cache is full at this time, EVICT_FIFO should evict seg 0 and item 0 */
    bstring_set_literal(&key, keys[4]);
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);
    ck_assert_int_eq(it->seg_id, 0);
    seg = item_to_seg(it);
    ck_assert_msg(seg->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->write_offset == item_ntotal(it) ||
                    seg->write_offset == item_ntotal(it) + 8,
            "write offset error %" PRIu32, seg->write_offset);
    ck_assert_msg(seg->write_offset == seg->occupied_size);
    ck_assert(seg->n_item == 1);

    /* double check item 1 is not in cache */
    bstring_set_literal(&key, keys[0]);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item should have been evicted");

#undef KEY
#undef VAL
}
END_TEST


START_TEST(test_segevict_CTE)
{
#define KEY "test_segevict_CTE"
#define VLEN (1000 * KiB)
#define MEM_SIZE "4194304"

    char *keys[] = {"cte-0", "cte-1", "cte-2", "cte-3", "cte-4", "cte-5",
            "cte-6", "cte-7", "cte-8"};
    seg_teardown();
    option_set(&options.seg_mem, MEM_SIZE);
    option_set(&options.seg_evict_opt, "3");
    seg_setup(&options, &metrics);

    struct bstring key, val;
    struct item *it;
    struct seg *seg;
    item_rstatus_e status;

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    for (uint32_t i = 0; i < 4; i++) {
        proc_sec++;
        bstring_set_literal(&key, keys[i]);
        status = item_reserve(
                &it, &key, &val, val.len, 0, proc_sec + 63 - 8 * i);
        ck_assert(it != NULL);

        item_insert(it);
        it = item_get(&key);
        ck_assert(it != NULL);
        item_release(it);
    }

    /* cache is full at this time, EVICT_CTE should evict seg 3 and item 3 */
    bstring_set_literal(&key, keys[4]);
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    ck_assert_int_eq(it->seg_id, 3);
    seg = item_to_seg(it);
    ck_assert_msg(seg->r_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->w_refcount == 0, "seg refcount incorrect");
    ck_assert_msg(seg->write_offset == item_ntotal(it) ||
                    seg->write_offset == item_ntotal(it) + 8,
            "write offset error %" PRIu32, seg->write_offset);
    ck_assert_msg(seg->write_offset == seg->occupied_size);
    ck_assert(seg->n_item == 1);

    /* double check item 3 is not in cache */
    bstring_set_literal(&key, keys[3]);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item should have been evicted");

#undef KEY
#undef VAL
}
END_TEST

START_TEST(test_segevict_UTIL)
{
#define KEY "test_segevict_UTIL"
#define VLEN_SMALL (500 * KiB)
#define VLEN_LARGE (1000 * KiB)
#define MEM_SIZE "4194304"

    char *keys[] = {"util-0", "util-1", "util-2", "util-3", "util-4", "util-5",
            "util-6", "util-7", "util-8"};
    seg_teardown();
    option_set(&options.seg_mem, MEM_SIZE);
    option_set(&options.seg_evict_opt, "4");
    seg_setup(&options, &metrics);
    proc_sec = 0;

    struct bstring key, val_small, val_large;
    struct item *it;
    struct seg *seg;
    item_rstatus_e status;

    val_small.data = cc_alloc(VLEN_SMALL);
    val_large.data = cc_alloc(VLEN_LARGE);
    cc_memset(val_small.data, 'A', VLEN_SMALL);
    cc_memset(val_large.data, 'A', VLEN_LARGE);
    val_small.len = VLEN_SMALL;
    val_large.len = VLEN_LARGE;

    for (uint32_t i = 0; i < 4; i++) {
        bstring_set_cstr(&key, keys[i]);
        status = item_reserve(
                &it, &key, &val_small, val_small.len, 0, INT32_MAX);
        ck_assert(it != NULL);
        item_insert(it);
    }

    /* first two segments are full with four small items,
     * now replace the last three of them */
    for (uint32_t i = 1; i < 4; i++) {
        bstring_set_cstr(&key, keys[i]);
        status = item_reserve(
                &it, &key, &val_small, val_small.len, 0, INT32_MAX);
        ck_assert(it != NULL);
        item_update(it);
    }

    /* three items are replaced,
     * seg 0 has small item 0;
     * seg 1 is empty;
     * seg 2 has small item 1 and 2;
     * seg 3 has small item 3 */
    ck_assert(heap.segs[1].write_offset > VLEN_SMALL);
    ck_assert(heap.segs[1].occupied_size < 200);
    ck_assert(heap.segs[1].n_item == 0);

    /* now we add one large item, seg 3 is too small to fit in,
     * so it will be sealed, and seg 1 will be evicted */
    bstring_set_cstr(&key, keys[4]);
    status = item_reserve(&it, &key, &val_large, val_large.len, 0, INT32_MAX);
    ck_assert(it != NULL);
    item_insert(it);

    /* check seg 3 */
    ck_assert(heap.segs[3].n_item == 1);

    /* check whether reserved item is on seg 1 */
    seg = item_to_seg(it);
    ck_assert_int_eq(seg->seg_id, 1);
    ck_assert(heap.segs[1].write_offset < heap.seg_size);
    ck_assert(heap.segs[1].occupied_size < heap.seg_size);
    ck_assert_int_eq(heap.segs[1].n_item, 1);

#undef KEY
#undef VAL
}
END_TEST


START_TEST(test_segevict_RAND)
{
#define KEY "test_segevict_RAND"
#define VLEN (1000 * KiB)
#define MEM_SIZE "4194304"

    char *keys[] = {"rand-0", "rand-1", "rand-2", "rand-3", "rand-4", "rand-5",
            "rand-6", "rand-7", "rand-8"};
    seg_teardown();
    option_set(&options.seg_mem, MEM_SIZE);
    option_set(&options.seg_evict_opt, "1");
    seg_setup(&options, &metrics);

    struct bstring key, val;
    struct item *it;
    struct seg *seg;
    item_rstatus_e status;

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    for (uint32_t i = 0; i < 160; i++) {
        it = NULL;
        bstring_set_literal(&key, keys[i % 8]);
        status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
        ck_assert(it != NULL);
        item_insert_or_update(it);

        it = item_get(&key);
        ck_assert_msg(it != NULL, "inserted %s but not found", keys[i % 8]);
        seg = item_to_seg(it);
        ck_assert_msg(seg->write_offset == item_ntotal(it) ||
                        seg->write_offset == item_ntotal(it) + 8,
                "write offset error %" PRIu32, seg->write_offset);
        ck_assert_msg(seg->write_offset == seg->occupied_size);
        ck_assert_int_eq(seg->n_item, 1);

        item_release(it);
    }
#undef KEY
#undef VAL
}
END_TEST

#ifdef do_not_define
START_TEST(test_segevict_SMART)
{
#    define KEY "test_segevict_SMART"
#    define VLEN (1000 * KiB)
#    define MEM_SIZE "4194304"

    char *keys[] = {"smart-1", "smart-2", "smart-3", "smart-4", "smart-5",
            "smart-6", "smart-7", "smart-8"};
    seg_teardown();
    option_set(&options.seg_mem_dram, MEM_SIZE);
    option_set(&options.seg_evict_opt, "4");
    seg_setup(&options, &metrics);

    struct bstring key, val;
    struct item *it;
    struct seg *seg;
    item_rstatus_e status;

    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;

    for (uint32_t i = 0; i < 4; i++) {
        bstring_set_literal(&key, keys[i]);
        status = item_reserve(&it, &key, &val, val.len, 0, 64 - 8 * i + 1);
        ck_assert(it != NULL);
        item_insert(it);
        it = item_get(&key);
        ck_assert(it != NULL);
        item_release(it);
    }

    /* cache is full at this time, EVICT_CTE should evict seg 3 and item 4 */
    bstring_set_literal(&key, keys[4]);
    status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    ck_assert_msg(status == ITEM_OK, "item_reserve not OK - return status %d",
            status);
    item_insert(it);

    ck_assert_int_eq(it->seg_id, 3);
    seg = item_to_seg(it) ck_assert_msg(seg->refcount == 0, "refcount not 0");
    ck_assert_msg(seg->write_offset == item_ntotal(it) ||
                    seg->write_offset == item_ntotal(it) + 8,
            "write offset error %" PRIu32, seg->write_offset);
    ck_assert_msg(seg->write_offset == seg->occupied_size);
    ck_assert(seg->n_item == 1);
    ck_assert(seg->sealed == 0);

    /* double check item 1 is not in cache */
    bstring_set_literal(&key, keys[3]);
    it = item_get(&key);
    ck_assert_msg(it == NULL, "item should have been evicted");

#    undef KEY
#    undef VAL
}
END_TEST
#endif

START_TEST(test_ttl_bucket_basic)
{
#define KEY "test_ttl_bucket_basic"
#define VLEN (1000 * KiB)
#define TIME 12345678

    test_reset();
    proc_sec = 0;

    struct bstring key, val;
    item_rstatus_e status;
    struct item *it, *it2;
    uint32_t offset, occu_size;

    key = str2bstr(KEY);
    val.data = cc_alloc(VLEN);
    cc_memset(val.data, 'A', VLEN);
    val.len = VLEN;


    struct seg *seg1, *seg2;
    for (uint32_t i = 0; i < 4; i++) {
        status = item_reserve(
                &it, &key, &val, val.len, 0, time_proc_sec() + 8 * i + 2);
        ck_assert_msg(status == ITEM_OK, "item_reserve not OK %d", status);
        seg1 = item_to_seg(it);
        ck_assert_msg(ttl_buckets[i].first_seg_id == seg1->seg_id,
                "ttl_bucket seg list not correct %" PRId32 " != %" PRId32,
                ttl_buckets[i].first_seg_id, seg1->seg_id);
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
        ck_assert_msg(ttl_buckets[i].first_seg_id == seg1->seg_id,
                "ttl_bucket seg list not correct %" PRId32 " != %" PRId32,
                ttl_buckets[i].first_seg_id, seg1->seg_id);
        ck_assert_msg(ttl_buckets[i].last_seg_id == seg2->seg_id,
                "ttl_bucket seg list not correct %" PRId32 " != %" PRId32,
                ttl_buckets[i].first_seg_id, seg1->seg_id);
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


/*
 * test suite
 */
static Suite *
seg_suite(void)
{
    Suite *s = suite_create(SUITE_NAME);

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
    tcase_add_test(tc_ttl, test_ttl_bucket_find);
    tcase_add_test(tc_ttl, test_ttl_bucket_basic);


    TCase *tc_seg = tcase_create("seg api");
    suite_add_tcase(s, tc_seg);
    tcase_add_test(tc_seg, test_seg_basic);
    tcase_add_test(tc_seg, test_seg_more);
    tcase_add_test(tc_seg, test_segevict_FIFO);
    tcase_add_test(tc_seg, test_segevict_CTE);
    tcase_add_test(tc_seg, test_segevict_UTIL);
    tcase_add_test(tc_seg, test_segevict_RAND);
    //    tcase_add_test(tc_seg, test_segevict_SMART);

    return s;
}

int
main(void)
{
    int nfail;

    /* setup */
    test_setup();

    /* turn on during debug */
    debug_options_st debug_opts = {DEBUG_OPTION(OPTION_INIT)};
    option_load_default(
            (struct option *)&debug_opts, OPTION_CARDINALITY(debug_options_st));
    debug_opts.debug_log_level.val.vuint = 6;
    debug_setup(&debug_opts);


    Suite *suite = seg_suite();
    SRunner *srunner = srunner_create(suite);
    srunner_set_log(srunner, DEBUG_LOG);
    srunner_run_all(srunner, CK_ENV); /* set CK_VEBOSITY in ENV to customize */
    nfail = srunner_ntests_failed(srunner);
    srunner_free(srunner);

    /* teardown */
    test_teardown();

    return (nfail == 0) ? EXIT_SUCCESS : EXIT_FAILURE;
}
