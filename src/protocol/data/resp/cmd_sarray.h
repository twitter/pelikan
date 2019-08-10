#pragma once

/**
 * create: create an empty array or integer width ESIZE
 * SArray.create KEY ESIZE
 *
 * delete: delete an array
 * SArray.delete KEY
 *
 * len: return number of entries in array
 * SArray.len KEY
 *
 * find: find (rank of an value) in array
 * SArray.find KEY VALUE
 *
 * get: get entry/entries at an index
 * SArray.get KEY [INDEX [COUNT]]
 *
 * insert: insert value
 * SArray.insert KEY VALUE [VALUE ...]
 *
 * remove: remove a particular value from array
 * SArray.remove KEY VALUE
 *
 * truncate: truncate a array
 * SArray.truncate KEY COUNT
 *
 */


/*          type                    string              #arg    #opt */
#define REQ_SARRAY(ACTION)                                          \
    ACTION( REQ_SARRAY_CREATE,      "SArray.create",    3,      0  )\
    ACTION( REQ_SARRAY_DELETE,      "SArray.delete",    2,      0  )\
    ACTION( REQ_SARRAY_LEN,         "SArray.len",       2,      0  )\
    ACTION( REQ_SARRAY_FIND,        "SArray.find",      3,      0  )\
    ACTION( REQ_SARRAY_GET,         "SArray.get",       2,      2  )\
    ACTION( REQ_SARRAY_INSERT,      "SArray.insert",    3,      -1 )\
    ACTION( REQ_SARRAY_REMOVE,      "SArray.remove",    3,      -1 )\
    ACTION( REQ_SARRAY_TRUNCATE,    "SArray.truncate",  3,      0  )

typedef enum sarray_elem {
    SARRAY_VERB = 0,
    SARRAY_KEY = 1,
    SARRAY_ESIZE = 2,
    SARRAY_VAL = 2,
    SARRAY_IDX = 2,
    SARRAY_CNT = 2,
    SARRAY_ICNT = 3, /* when an index is also present */
} sarray_elem_e;
