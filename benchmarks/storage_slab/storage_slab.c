#include <bench_storage.h>

#include <storage/slab/item.h>
#include <storage/slab/slab.h>

static slab_metrics_st metrics = { SLAB_METRIC(METRIC_INIT) };

unsigned
bench_storage_config_nopts(void)
{
    return OPTION_CARDINALITY(slab_options_st);
}

void
bench_storage_config_init(void *options)
{
    slab_options_st *opts = options;
    *opts = (slab_options_st){ SLAB_OPTION(OPTION_INIT) };

    option_load_default(options, OPTION_CARDINALITY(slab_options_st));
}

rstatus_i
bench_storage_init(void *opts, size_t item_size, size_t nentries)
{
    slab_options_st *options = opts;
    if (item_size != 0 && nentries != 0){
        /* Q(jason): should SLAB_MEM be SLAB_SIZE? */
    options->slab_mem.val.vuint =
        CC_ALIGN((ITEM_HDR_SIZE + item_size) * nentries, SLAB_SIZE);
    options->slab_item_min.val.vuint = item_size;
    }

    slab_setup(options, &metrics);

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
    struct bstring key = {.data=e->key, .len=e->key_len};
    struct bstring val = {.data=e->val, .len=e->val_len};
    struct item *it;

    item_rstatus_e status = item_reserve(&it, &key, &val, val.len, 0, e->ttl);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it, &key);

    return CC_OK;
}

rstatus_i
bench_storage_get(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};

    struct item *it = item_get(&key);

    return it != NULL ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_rem(struct benchmark_entry *e)
{
    struct bstring key = {.data=e->key, .len=e->key_len};

    return item_delete(&key) ? CC_OK : CC_EEMPTY;
}
