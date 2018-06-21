struct CDBHandle;

struct CDBBString {
    uint32_t len;   /* string length */
    char     *data; /* string data */
};


struct CDBHandle* cdb_handle_create(const char *path);
void cdb_handle_destroy(struct CDBHandle *h);
void cdb_bstring_destroy(struct CDBBString *b);
struct CDBBString* cdb_get_h(struct CDBHandle *h, const struct CDBBString *key);
struct CDBBString* cdb_get(const struct CDBBString *key);
void cdb_setup(const char *path);
void cdb_teardown(void);
