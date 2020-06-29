#pragma once

#include <cc_define.h>
#include <stddef.h>

typedef size_t benchmark_key_u;

typedef enum {
    op_get = 0,
    op_gets,
    op_set,
    op_add,
    op_cas,
    op_replace,
    op_append,
    op_prepend,
    op_delete,
    op_incr,
    op_decr,

    op_invalid
} op_e;

static const char *op_names[op_invalid] = {"get", "gets", "set", "add", "cas",
                                           "replace", "append", "prepend", "delete", "incr", "decr"};

struct benchmark_entry {
    char *key;
    benchmark_key_u key_len;
    char *val;
    size_t val_len;
    op_e op;
};

rstatus_i bench_storage_init(void *opts, size_t item_size, size_t nentries);
rstatus_i bench_storage_deinit(void);
rstatus_i bench_storage_put(struct benchmark_entry *e);
rstatus_i bench_storage_get(struct benchmark_entry *e);
rstatus_i bench_storage_rem(struct benchmark_entry *e);
unsigned bench_storage_config_nopts(void);
void bench_storage_config_init(void *opts);

