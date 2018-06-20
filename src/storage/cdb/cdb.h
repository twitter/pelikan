
struct CDBHandle;

*CDBHandle cdb_create(*const char);
void cdb_destroy(*CDBHandle);
*char cdb_get(*CDBHandle, *char);
