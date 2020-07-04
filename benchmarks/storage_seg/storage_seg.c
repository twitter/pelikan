#include <bench_storage.h>

#include <storage/seg/item.h>
#include <storage/seg/seg.h>

#include <storage/seg/hashtable.h>
extern struct hash_table *hash_table;


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
        /* because we don't update in-place, we need to allocate large enough space */
        options->seg_mem.val.vuint =
                CC_ALIGN((ITEM_HDR_SIZE + item_size) * nentries * 40, SEG_SIZE);
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
    struct bstring key = {.data=e->key, .len=e->key_len};

    struct item *it = item_get(&key);

    rstatus_i status = it != NULL ? CC_OK : CC_EEMPTY;

    if (it){
        /* TODO(jason): copy the data ? */
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

    struct item *it = item_get(&key);
    rstatus_i status = item_incr(&vint, it, e->delta) == ITEM_OK? CC_OK : CC_ERROR;
    item_release(it);

    return status;
}

rstatus_i
bench_storage_decr(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    uint64_t vint;

    struct item *it = item_get(&key);
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

    item_insert_or_update(it);

    return CC_OK;
}

rstatus_i
bench_storage_add(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct item *it;

    it = item_check_existence(&key);
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

    oit = item_check_existence(&key);
    if (oit == NULL){
        return CC_ERROR;
    }

//    uint32_t cas = item_get_cas(it);

    struct bstring val = {.data=e->val, .len=e->val_len};
    item_rstatus_e status = item_reserve(&nit, &key, &val, val.len, 0, e->expire_at);

    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert_or_update(nit);

    return CC_OK;
}

rstatus_i
bench_storage_replace(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct item *it;

    it = item_check_existence(&key);
    if (it == NULL){
        return CC_OK;
    }

    struct bstring val = {.data=e->val, .len=e->val_len};
    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, e->expire_at);

    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_update(it);

    return CC_OK;
}
