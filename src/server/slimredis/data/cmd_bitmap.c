#include "process.h"

#include "data_structure/bitmap/bitset.h"
#include "protocol/data/redis_include.h"
#include "storage/cuckoo/cuckoo.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>


static inline struct bstring *
_get_key(struct request *req)
{
    struct element *key = (struct element *)array_get(req->token, 1);

    return &key->bstr;
}

static inline uint16_t
_get_col(struct response *rsp, struct request *req)
{
    uint64_t col = 0;
    rstatus_i status;
    struct element *reply = (struct element *)array_get(rsp->token, 1);
    struct element *arg = (struct element *)array_get(req->token, 2);

    status = bstring_atou64(&col, &arg->bstr);
    if (status != CC_OK || col > BITSET_COL_MAX) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_ERR_ARG);
        log_warn("%.*s is not a valid value for bitmap size", arg->bstr.len,
                arg->bstr.data);
        INCR(process_metrics, process_ex);
        INCR(process_metrics, bitmap_create_ex);
    }
    return (uint16_t)col;
}

static inline struct item *
_add_key(struct response *rsp, struct bstring *key)
{
    struct element *reply = (struct element *)array_get(rsp->token, 1);
    struct val val = {VAL_TYPE_STR, {null_bstring}};
    struct item *it;

    it = cuckoo_get(key);
    if (it != NULL) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, bitmap_create_exist);

        return NULL;
    } else { /* cuckoo insert current won't fail as long as size is valid */
        it = cuckoo_insert(key, &val, 0);
        if (it == NULL) {
            rsp->type = reply->type = ELEM_ERR;
            reply->bstr = str2bstr(RSP_ERR_STORAGE);
            INCR(process_metrics, bitmap_create_ex);
            INCR(process_metrics, process_ex);
        } else {
            INCR(process_metrics, bitmap_create_stored);
        }

        return it;
    }
}

void
cmd_bitmap_create(struct response *rsp, struct request *req, struct command *cmd)
{
    struct item *it;
    uint16_t ncol;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);

    INCR(process_metrics, bitmap_create);

    /* check column size first so we don't have to undo storage op if invalid */
    ncol = _get_col(rsp, req);
    if (ncol == 0) {
        log_debug("command '%.*s' '%.*s' failed: invalid arg", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        return;
    }

    it = _add_key(rsp, key);
    if (it == NULL) {
        log_debug("command '%.*s' '%.*s' failed: cannot store", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        return;
    }

    /* initialize data structure */
    bitset_init((struct bitset *)ITEM_VAL_POS(it), ncol);

    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_OK);

    log_verb("command '%.*s' '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
}

void
cmd_bitmap_delete(struct response *rsp, struct request *req, struct command *cmd)
{
    struct element *el;
    struct bstring *key;
    bool status;

    INCR(process_metrics, bitmap_delete);

    key = _get_key(req);

    rsp->type = ELEM_STR;
    el = array_push(rsp->token);
    ASSERT(el != NULL); /* cannot fail because we preallocate tokens */
    el->type = ELEM_STR;
    status = cuckoo_delete(key);
    if (status) {
        el->bstr = str2bstr(RSP_OK);
        INCR(process_metrics, bitmap_delete_deleted);
    } else {
        el->bstr = str2bstr(RSP_NOT_FOUND);
        INCR(process_metrics, bitmap_delete_notfound);
    }

    log_verb("command '%.*s' on key '%.*s' returned %d", cmd->bstr.len,
            cmd->bstr.data, key->len, key->data, status);
}
