#include "attribute.h"

#define GET_BSTR(_type, _str) str2bstr(_str),
struct bstring attrib_table[ATTRIB_SENTINEL] = {
    null_bstring,
    ATTRIB_GLOBAL(GET_BSTR)
};
#undef GET_BSTR

