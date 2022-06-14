#pragma once

#include "cmd.h"

/**
 * create: create an empty list
 * List.create KEY
 *
 * delete: delete a list or a particular value from list
 * List.delete KEY [VALUE [COUNT]]
 *
 * trim: trimming a list
 * List.trim KEY INDEX COUNT
 *
 * len: return number of entries in list
 * List.len KEY
 *
 * find: find entry in list
 * List.find KEY VALUE
 *
 * get: get entry/entries at an index
 * List.get KEY [INDEX [COUNT]]
 *
 * insert: insert entry at an index
 * List.insert KEY VALUE INDEX
 *
 * push: pushing entry/entries at the end
 * List.push KEY VALUE [VALUE ...]
 */


/*          type                string          #arg    #opt */
#define REQ_LIST(ACTION)                                            \
    ACTION( REQ_LIST_CREATE,  "List.create",    2,      0          )\
    ACTION( REQ_LIST_DELETE,  "List.delete",    2,      2          )\
    ACTION( REQ_LIST_LEN,     "List.len",       2,      0          )\
    ACTION( REQ_LIST_FIND,    "List.find",      3,      0          )\
    ACTION( REQ_LIST_GET,     "List.get",       2,      2          )\
    ACTION( REQ_LIST_INSERT,  "List.insert",    4,      0          )\
    ACTION( REQ_LIST_PUSH,    "List.push",      3,      OPT_VARIED )\
    ACTION( REQ_LIST_TRIM,    "List.trim",      4,      0          )

typedef enum list_elem {
    LIST_KEY = 2,
    LIST_VAL = 3,
    LIST_IDX = 3,
    LIST_VIDX = 4, /* when a value is also present */
    LIST_CNT = 4,
} list_elem_e;
