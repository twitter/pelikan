#include <bench_storage.h>

#include <storage/slab/item.h>
#include <storage/slab/slab.h>

static slab_metrics_st metrics = { SLAB_METRIC(METRIC_INIT) };
static slab_options_st options = { SLAB_OPTION(OPTION_INIT) };

rstatus_i
bench_storage_init(size_t item_size, size_t nentries)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.slab_evict_opt.val.vuint = EVICT_NONE;
    slab_setup(&options, &metrics);

    return CC_OK;
}

rstatus_i
bench_storage_deinit(void)
{
    slab_teardown();
    return CC_OK;
}

rstatus_i
bench_storage_put(struct benchmark_entry *e)
{
    struct bstring key;
    struct bstring val;
    struct item *it;

    bstring_set_cstr(&val, e->value);
    bstring_set_cstr(&key, e->key);

    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, INT32_MAX);

    item_insert(it, &key);

    return status == ITEM_OK ? CC_OK : CC_ENOMEM;
}

rstatus_i
bench_storage_get(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);
    struct item *it = item_get(&key);

    return it != NULL ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_rem(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);

    return item_delete(&key) ? CC_OK : CC_EEMPTY;
}
