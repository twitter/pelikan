#pragma once

#include <cc_bstring.h>

#include <stdint.h>

struct CDBHandle;

struct CDBBString {
    uint32_t len;   /* string length */
    char     *data; /* string data */
};

struct CDBData;

struct CDBHandle* cdb_handle_create(const char *path);
void cdb_handle_destroy(struct CDBHandle *h);

void cdb_setup(void);
void cdb_teardown(void);

struct bstring *cdb_get(struct CDBHandle *h, struct bstring *key, struct bstring *value);
