#pragma once

#include <cc_define.h>
#include <stddef.h>

typedef size_t benchmark_key_u;

struct benchmark_entry {
    char *key;
    benchmark_key_u key_size;
    char *value;
    size_t value_size;
};

rstatus_i bench_storage_init(size_t item_size, size_t nentries);
rstatus_i bench_storage_deinit(void);
rstatus_i bench_storage_put(struct benchmark_entry *e);
rstatus_i bench_storage_get(struct benchmark_entry *e);
rstatus_i bench_storage_rem(struct benchmark_entry *e);
