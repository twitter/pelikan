#pragma once

#include "cmd.h"

/**
 * KEY: key used to represet the sorted map
 * IKEY: integer key that is used for sorting within a map
 * VALUE: fixed width value that is associated with an IKEY
 * COUNT: number of elements, can be negative which indicates right to left
 */

/**
 * create: create an empty map or integer width ESIZE & value width VSIZE
 * SMap.create KEY ISIZE VSIZE [WATERMARK_L] [WATERMARK_H]
 *
 * delete: delete an map
 * SMap.delete KEY
 *
 * len: return number of entries in map
 * SMap.len KEY
 *
 * find: find (rank of an ikey) in map
 * SMap.find KEY IKEY
 *
 * get: get entry/entries at an index
 * SMap.get KEY [INDEX [COUNT]]
 *
 * insert: insert ikey
 * SMap.insert KEY IKEY VALUE [IKEY VALUE ...]
 *
 * remove: remove a particular ikey from map
 * SMap.remove KEY IKEY [IKEY ...]
 *
 * truncate: truncate a map
 * SMap.truncate KEY COUNT
 *
 */


/*          type                string              #arg    #opt */
#define REQ_SMAP(ACTION)                                                \
    ACTION( REQ_SMAP_CREATE,    "SMap.create",      3,      2          )\
    ACTION( REQ_SMAP_DELETE,    "SMap.delete",      2,      0          )\
    ACTION( REQ_SMAP_LEN,       "SMap.len",         2,      0          )\
    ACTION( REQ_SMAP_FIND,      "SMap.find",        3,      0          )\
    ACTION( REQ_SMAP_GET,       "SMap.get",         2,      2          )\
    ACTION( REQ_SMAP_INSERT,    "SMap.insert",      3,      OPT_VARIED )\
    ACTION( REQ_SMAP_REMOVE,    "SMap.remove",      3,      OPT_VARIED )\
    ACTION( REQ_SMAP_TRUNCATE,  "SMap.truncate",    3,      0          )

typedef enum smap_elem {
    SMAP_KEY = 2,
    SMAP_ISIZE = 3,
    SMAP_VSIZE = 4,
    SMAP_IKEY = 3,
    SMAP_IDX = 3,
    SMAP_CNT = 3,
    SMAP_ICNT = 4, /* when an index is also present */
    SMAP_WML = 5,  /* watermark (low) */
    SMAP_WMH = 6,  /* watermark (high) */
} smap_elem_e;
