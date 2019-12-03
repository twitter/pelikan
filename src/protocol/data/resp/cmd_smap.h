#pragma once

/**
 * KEY: key used to represet the sorted map
 * IKEY: integer key that is used for sorting within a map
 * VALUE: fixed width value that is associated with an IKEY
 * COUNT: number of elements, can be negative which indicates right to left
 */

/**
 * create: create an empty map or integer width ESIZE & value width VSIZE
 * SMap.create KEY ESIZE VSIZE [WATERMARK_L] [WATERMARK_H]
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
#define REQ_SMAP(ACTION)                                            \
    ACTION( REQ_SMAP_CREATE,    "SMap.create",      3,      2  )\
    ACTION( REQ_SMAP_DELETE,    "SMap.delete",      2,      0  )\
    ACTION( REQ_SMAP_LEN,       "SMap.len",         2,      0  )\
    ACTION( REQ_SMAP_FIND,      "SMap.find",        3,      0  )\
    ACTION( REQ_SMAP_GET,       "SMap.get",         2,      2  )\
    ACTION( REQ_SMAP_INSERT,    "SMap.insert",      3,      -1 )\
    ACTION( REQ_SMAP_REMOVE,    "SMap.remove",      3,      -1 )\
    ACTION( REQ_SMAP_TRUNCATE,  "SMap.truncate",    3,      0  )

typedef enum smap_elem {
    SMAP_VERB = 0,
    SMAP_KEY = 1,
    SMAP_ESIZE = 2,
    SMAP_IKEY = 2,
    SMAP_IDX = 2,
    SMAP_CNT = 2,
    SMAP_VSIZE = 3,
    SMAP_VAL = 3,
    SMAP_ICNT = 3, /* when an index is also present */
    SMAP_WML = 3,  /* watermark (low) */
    SMAP_WMH = 4,  /* watermark (high) */
} smap_elem_e;
