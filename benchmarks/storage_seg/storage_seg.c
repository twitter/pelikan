#include <bench_storage.h>

#include <storage/seg/item.h>
#include <storage/seg/seg.h>

#include <math.h>

#define VERIFY_DATA

static seg_metrics_st metrics = {SEG_METRIC(METRIC_INIT)};

unsigned
bench_storage_config_nopts(void)
{
    return OPTION_CARDINALITY(seg_options_st);
}

void
bench_storage_config_init(void *options)
{
    seg_options_st *opts = options;
    *opts = (seg_options_st){SEG_OPTION(OPTION_INIT)};

    option_load_default(options, OPTION_CARDINALITY(seg_options_st));
}

rstatus_i
bench_storage_init(void *opts, size_t item_size, size_t nentries)
{
    seg_options_st *options = opts;
    if (item_size != 0 && nentries != 0) {
        /* because we don't update in-place, we need to allocate large enough space
         * note that the old bench does not work well with seg */
        options->heap_mem.val.vuint =
                CC_ALIGN((ITEM_HDR_SIZE + item_size) * nentries, SEG_SIZE);
        if (options->heap_mem.val.vuint < 1UL * GiB) {
            options->heap_mem.val.vuint = 1UL * GiB;
        }
//        printf("item size max %zu, seg allocate %.2lf GB heap\n",
//                item_size, (double) options->seg_mem.val.vuint/GiB);

        /* also update hash table hash power */
        options->hash_power.val.vuint = (uint64_t)(ceil(log2(nentries)));
    }

    seg_setup(options, &metrics);

    return CC_OK;
}

rstatus_i
bench_storage_deinit(void)
{
    seg_teardown();
    return CC_OK;
}


rstatus_i
bench_storage_get(struct benchmark_entry *e)
{
    static __thread char data[1 * MiB];
    struct bstring key = {.data=e->key, .len=e->key_len};

    struct item *it = item_get(&key, NULL, true);

    rstatus_i status = it != NULL ? CC_OK : CC_EEMPTY;

    if (it){
#ifdef VERIFY_DATA
        memcpy(data, item_val(it), item_nval(it));
        ASSERT(e->key_len == it->klen);
        ASSERT(memcmp(e->key, item_key(it), e->key_len) == 0);
        ASSERT(memcmp(data, "ABCDEF", MIN(item_nval(it), 6)) == 0);
#endif
        item_release(it);
    }

    return status;
}

rstatus_i
bench_storage_gets(struct benchmark_entry *e)
{
    /* this is currently not implemented */

    return bench_storage_get(e);
}


rstatus_i
bench_storage_delete(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};

    return item_delete(&key) ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_incr(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    uint64_t vint;

    struct item *it = item_get(&key, NULL, true);
    rstatus_i status = item_incr(&vint, it, e->delta) == ITEM_OK? CC_OK : CC_ERROR;
    item_release(it);

    return status;
}

rstatus_i
bench_storage_decr(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    uint64_t vint;

    struct item *it = item_get(&key, NULL, true);
    rstatus_i status = item_decr(&vint, it, e->delta) == ITEM_OK? CC_OK : CC_ERROR;

    item_release(it);

    return status;
}

rstatus_i
bench_storage_set(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct bstring val = {.data=e->val, .len=e->val_len};
    struct item *it;

    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, e->expire_at);
    if (status != ITEM_OK)
        return CC_ENOMEM;

#ifdef VERIFY_DATA
    ASSERT(e->key_len == it->klen);
    ASSERT(memcmp(e->key, item_key(it), e->key_len) == 0);
    ASSERT(memcmp(item_val(it), "ABCDEF", MIN(item_nval(it), 6)) == 0);
#endif

    item_insert(it);

    return CC_OK;
}

rstatus_i
bench_storage_add(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct item *it;

    it = item_get(&key, NULL, false);
    if (it != NULL){
        return CC_OK;
    }

    struct bstring val = {.data=e->val, .len=e->val_len};
    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, e->expire_at);

    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it);

    return CC_OK;
}

rstatus_i
bench_storage_cas(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct item *oit, *nit;
    uint64_t old_cas, new_cas;

    oit = item_get(&key, &old_cas, false);
    if (oit == NULL){
        return CC_ERROR;
    }

    struct bstring val = {.data=e->val, .len=e->val_len};
    item_rstatus_e status = item_reserve(&nit, &key, &val, val.len, 0, e->expire_at);

    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(nit);
    oit = item_get(&key, &new_cas, false);

    ASSERT(old_cas != new_cas);

    return CC_OK;
}

rstatus_i
bench_storage_replace(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct item *it;

    it = item_get(&key, NULL, false);
    if (it == NULL){
        return CC_OK;
    }

    struct bstring val = {.data=e->val, .len=e->val_len};
    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, e->expire_at);

    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it);

    return CC_OK;
}
