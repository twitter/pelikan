#include <bench_storage.h>

#include <storage/slab/item.h>
#include <storage/slab/slab.h>
#include <cc_print.h>

static slab_metrics_st metrics = { SLAB_METRIC(METRIC_INIT) };

#define VERIFY_DATA

unsigned
bench_storage_config_nopts(void)
{
    return OPTION_CARDINALITY(slab_options_st);
}

void
bench_storage_config_init(void *options)
{
    slab_options_st *opts = options;
    *opts = (slab_options_st){SLAB_OPTION(OPTION_INIT)};

    option_load_default(options, OPTION_CARDINALITY(slab_options_st));
}

rstatus_i
bench_storage_init(void *opts, size_t item_size, size_t nentries)
{
    slab_options_st *options = opts;
    if (item_size != 0 && nentries != 0) {
        /* Q(jason): should SLAB_MEM be SLAB_SIZE? */
        /* double the size here because we may need more than
         * (ITEM_HDR_SIZE + item_size) * nentries due to internal fragmentation
         */
        options->slab_mem.val.vuint =
                CC_ALIGN((ITEM_HDR_SIZE + item_size) * nentries * 2, SLAB_SIZE);
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
bench_storage_get(struct benchmark_entry *e)
{
    static __thread char data[1*MiB];
    struct bstring key = {.data = e->key, .len = e->key_len};

    struct item *it = item_get(&key);

    rstatus_i status = it != NULL ? CC_OK : CC_EEMPTY;

    if (it){
        memcpy(data, item_data(it), item_nval(it));
#ifdef VERIFY_DATA
        ASSERT(e->key_len == it->klen);
        ASSERT(memcmp(e->key, item_key(it), e->key_len) == 0);
        ASSERT(memcmp(data, "ABCDEF", MIN(item_nval(it), 6)) == 0);
#endif
    }

    return status;
}

rstatus_i
bench_storage_gets(struct benchmark_entry *e)
{
    return bench_storage_get(e);
}

rstatus_i
bench_storage_delete(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};

    return item_delete(&key) ? CC_OK : CC_EEMPTY;
}

rstatus_i
bench_storage_set(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct bstring val = {.data = e->val, .len = e->val_len};
    struct item *it;

    item_rstatus_e status =
            item_reserve(&it, &key, &val, val.len, 0, e->expire_at);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it, &key);

    return CC_OK;
}

rstatus_i
bench_storage_add(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct item *it;

    it = item_get(&key);
    if (it != NULL) {
        return CC_OK;
    }

    struct bstring val = {.data = e->val, .len = e->val_len};
    item_rstatus_e status =
            item_reserve(&it, &key, &val, val.len, 0, e->expire_at);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it, &key);

    return CC_OK;
}

rstatus_i
bench_storage_replace(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct item *it;

    it = item_get(&key);
    if (it == NULL) {
        return CC_OK;
    }

    struct bstring val = {.data = e->val, .len = e->val_len};
    item_rstatus_e status =
            item_reserve(&it, &key, &val, val.len, 0, e->expire_at);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it, &key);

    return CC_OK;
}

rstatus_i
bench_storage_cas(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct item *it;

    it = item_get(&key);
    if (it == NULL) {
        return CC_ERROR;
    }

    __attribute__((unused)) uint64_t cas = item_get_cas(it);

    struct bstring val = {.data = e->val, .len = e->val_len};
    item_rstatus_e status =
            item_reserve(&it, &key, &val, val.len, 0, e->expire_at);
    if (status != ITEM_OK)
        return CC_ENOMEM;

    item_insert(it, &key);

    return CC_OK;
}

rstatus_i
bench_storage_incr(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct item *it;
    item_rstatus_e status;
    uint64_t vint;
    struct bstring nval;
    char buf[CC_UINT64_MAXLEN];

    it = item_get(&key);
    if (it == NULL) {
        return CC_ERROR;
    }

    status = item_atou64(&vint, it);
    if (status != ITEM_OK) {
        return status;
    }

    nval.len = cc_print_uint64_unsafe(buf, vint);
    nval.data = buf;
    if (item_slabid(it->klen, nval.len, it->olen) == it->id) {
        item_update(it, &nval);
        return ITEM_OK;
    }

    status = item_reserve(&it, &key, &nval, nval.len, 0, it->expire_at);
    if (status == ITEM_OK) {
        item_insert(it, &key);
    }

    return CC_OK;
}

rstatus_i
bench_storage_decr(struct benchmark_entry *e)
{
    struct bstring key = {.data = e->key, .len = e->key_len};
    struct item *it;
    item_rstatus_e status;
    uint64_t vint;
    struct bstring nval;
    char buf[CC_UINT64_MAXLEN];

    it = item_get(&key);
    if (it == NULL) {
        return CC_ERROR;
    }

    status = item_atou64(&vint, it);
    if (status != ITEM_OK) {
        return status;
    }

    if (vint < e->delta) {
        vint = 0;
    } else {
        vint -= e->delta;
    }

    nval.len = cc_print_uint64_unsafe(buf, vint);
    nval.data = buf;
    if (item_slabid(it->klen, nval.len, it->olen) == it->id) {
        item_update(it, &nval);
        return ITEM_OK;
    }

    status = item_reserve(&it, &key, &nval, nval.len, 0, it->expire_at);
    if (status == ITEM_OK) {
        item_insert(it, &key);
    }

    return CC_OK;
}
