struct CDBHandle;

struct CDBBString {
    uint32_t len;   /* string length */
    char     *data; /* string data */
};


CDBHandle* cdb_create(const char *path);
void cdb_destroy(CDBHandle *h);
uint8_t* cdb_get(CDBHandle *h, const uint8_t *key);
