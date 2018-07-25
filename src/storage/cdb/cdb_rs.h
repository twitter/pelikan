#pragma once

#include <cc_bstring.h>

#include <stdint.h>

struct cdb_handle;

typedef enum cdb_load_method {
    CDB_HEAP = 1,
    CDB_MMAP = 2,
} cdb_load_method_e;

struct cdb_handle_create_config {
    struct bstring    *path;
    cdb_load_method_e load_method;
};


struct cdb_handle *cdb_handle_create(const struct cdb_handle_create_config *cfg);
void cdb_handle_destroy(struct cdb_handle **h);

void cdb_setup(void);
void cdb_teardown(void);

struct bstring *cdb_get(struct cdb_handle *h, struct bstring *key, struct bstring *value);
