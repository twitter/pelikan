#include <bench_storage.h>

#include <storage/seg/item.h>
#include <storage/seg/seg.h>

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
    /* because we don't update in-place, we need to allocate large enough space */
    options->seg_mem_dram.val.vuint =
            CC_ALIGN((ITEM_HDR_SIZE + item_size) * nentries * 24, SEG_SIZE);

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
bench_storage_put(struct benchmark_entry *e)
{
    struct bstring key;
    struct bstring val;
    struct item *it;

    bstring_set_cstr(&val, e->val);
    bstring_set_cstr(&key, e->key);
//    printf("put %s\n", e->key);

    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it);

    return CC_OK;
}

rstatus_i
bench_storage_get(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);
    struct item *it = item_get(&key);

    rstatus_i status = it != NULL ? CC_OK : CC_EEMPTY;

    if (it){
        item_release(it);
    }

    return status;
}

rstatus_i
bench_storage_rem(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);
//    printf("remove %s\n", e->key);

    return item_delete(&key) ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_incr(struct benchmark_entry *e)
{
    struct bstring key;
    uint64_t vint;
    bstring_set_cstr(&key, e->key);

    struct item *it = item_get(&key);
    rstatus_i status = item_incr(&vint, it, 20) == ITEM_OK? CC_OK : CC_ERROR;

    return status;
}
