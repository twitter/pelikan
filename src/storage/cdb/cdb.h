#pragma once

#include <cc_bstring.h>

#include <stdint.h>

struct CDBHandle;

enum CDBStoreMethod {
	CDB_HEAP = 1,
	CDB_MMAP = 2,
};

struct CDBHandle* cdb_handle_create(const char *path, enum CDBStoreMethod meth);

void cdb_handle_destroy(struct CDBHandle *h);

void cdb_setup(void);
void cdb_teardown(void);

struct bstring *cdb_get(struct CDBHandle *h, struct bstring *key, struct bstring *value);
