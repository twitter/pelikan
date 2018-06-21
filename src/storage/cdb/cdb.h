struct CDBHandle;

struct CDBBString {
    uint32_t len;   /* string length */
    char     *data; /* string data */
};


struct CDBHandle* cdb_create(const char *path);
void cdb_destroy(struct CDBHandle *h);
uint8_t* cdb_get(struct CDBHandle *h, const struct CDBBString *key);
void cdb_setup(void);
