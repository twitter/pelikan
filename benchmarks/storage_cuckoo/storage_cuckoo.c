#include <bench_storage.h>

#include <storage/cuckoo/item.h>
#include <storage/cuckoo/cuckoo.h>

static cuckoo_metrics_st metrics = { CUCKOO_METRIC(METRIC_INIT) };
static cuckoo_options_st options = { CUCKOO_OPTION(OPTION_INIT) };

rstatus_i
bench_storage_init(size_t item_size, size_t nentries)
{
    option_load_default((struct option *)&options, OPTION_CARDINALITY(options));
    options.cuckoo_policy.val.vuint = CUCKOO_POLICY_EXPIRE;
    options.cuckoo_item_size.val.vuint = item_size + ITEM_OVERHEAD;
    options.cuckoo_nitem.val.vuint = nentries;

    cuckoo_setup(&options, &metrics);

    return CC_OK;
}

rstatus_i
bench_storage_deinit(void)
{
    cuckoo_teardown();
    return CC_OK;
}

rstatus_i
bench_storage_put(struct benchmark_entry *e)
{
    struct bstring key;
    struct val val;
    val.type = VAL_TYPE_STR;
    bstring_set_cstr(&val.vstr, e->value);
    bstring_set_cstr(&key, e->key);

    struct item *it = cuckoo_insert(&key, &val, INT32_MAX);

    return it != NULL ? CC_OK : CC_ENOMEM;
}

rstatus_i
bench_storage_get(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);
    struct item *it = cuckoo_get(&key);

    return it != NULL ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_rem(struct benchmark_entry *e)
{
    struct bstring key;
    bstring_set_cstr(&key, e->key);

    return cuckoo_delete(&key) ? CC_OK : CC_EEMPTY;
}
