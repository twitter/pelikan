#pragma once

#include <cc_bstring.h>

/* Top level attributes */

/*          type            string */
#define ATTRIB_GLOBAL(ACTION)       \
    ACTION( ATTRIB_TTL,     "ttl"  )\
    ACTION( ATTRIB_FLAG,    "flag" )


#define GET_TYPE(_type, _str) _type,
typedef enum attrib_type {
    ATTRIB_UNKNOWN,
    ATTRIB_GLOBAL(GET_TYPE)
    ATTRIB_SENTINEL
} attrib_type_e;
#undef GET_TYPE

extern struct bstring attrib_table[ATTRIB_SENTINEL];

