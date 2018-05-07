#include "process.h"

#include "data_structure/bitmap/bitset.h"
#include "protocol/data/redis_include.h"
#include "storage/cuckoo/cuckoo.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>


#define it2bitset(_it) ((struct bitset *)ITEM_VAL_POS(it))

static inline struct bstring *
_get_key(struct request *req)
{
    struct element *key = (struct element *)array_get(req->token, BITMAP_KEY);

    return &key->bstr;
}

static inline int32_t
_get_col(struct response *rsp, struct request *req, uint16_t max)
{
    uint64_t col = 0;
    rstatus_i status;
    struct element *reply = (struct element *)array_get(rsp->token, BITMAP_VERB);
    struct element *arg = (struct element *)array_get(req->token, BITMAP_COL);

    status = bstring_atou64(&col, &arg->bstr);
    if (status != CC_OK || col > max) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_ERR_ARG);
        log_warn("%.*s is not a valid value column id/size", arg->bstr.len,
                arg->bstr.data);
        INCR(process_metrics, process_ex);

        return -1;
    }
    return (int32_t)col;
}

static inline int16_t
_get_bitval(struct response *rsp, struct request *req, uint8_t max)
{
    uint64_t val = 0;
    rstatus_i status;
    struct element *reply = (struct element *)array_get(rsp->token, BITMAP_VERB);
    struct element *arg = (struct element *)array_get(req->token, BITMAP_VAL);

    status = bstring_atou64(&val, &arg->bstr);
    if (status != CC_OK || val > max) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_ERR_ARG);
        log_warn("%.*s is not a valid bitmap field value", arg->bstr.len,
                arg->bstr.data);
        INCR(process_metrics, process_ex);

        return -1;
    }
    return (int16_t)val;
}

static inline struct item *
_add_key(struct response *rsp, struct bstring *key)
{
    struct element *reply = (struct element *)array_get(rsp->token, BITMAP_VERB);
    struct val val = {VAL_TYPE_STR, {null_bstring}};
    struct item *it;

    it = cuckoo_get(key);
    if (it != NULL) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, bitmap_create_exist);

        return NULL;
    } else { /* cuckoo insert current won't fail as long as size is valid */
        it = cuckoo_insert(key, &val, time_reltime(0));
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
    int32_t ncol;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);

    INCR(process_metrics, bitmap_create);

    /* check column size first so we don't have to undo storage op if invalid */
    ncol = _get_col(rsp, req, BITSET_COL_MAX);
    if (ncol <= 0) {
        log_debug("command '%.*s' '%.*s' failed: invalid arg", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_create_ex);
        return;
    }

    it = _add_key(rsp, key);
    if (it == NULL) {
        log_debug("command '%.*s' '%.*s' failed: cannot store", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        return;
    }

    /* initialize data structure */
    bitset_init((struct bitset *)ITEM_VAL_POS(it), (uint16_t)ncol);

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
        el->bstr = str2bstr(RSP_NOTFOUND);
        INCR(process_metrics, bitmap_delete_notfound);
    }

    log_verb("command '%.*s' on key '%.*s' returned %d", cmd->bstr.len,
            cmd->bstr.data, key->len, key->data, status);
}

void
cmd_bitmap_get(struct response *rsp, struct request *req, struct command *cmd)
{
    struct item *it;
    int32_t col;
    uint8_t val;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bitset *bs;

    INCR(process_metrics, bitmap_set);

    it = cuckoo_get(key);
    if (it == NULL) {
        rsp->type = reply->type = ELEM_STR;
        reply->bstr = str2bstr(RSP_NOTFOUND);
        log_verb("command '%.*s' on key '%.*s' : key not found", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_get_notfound);

        return;
    }

    bs = it2bitset(it);

    col = _get_col(rsp, req, size2bit(bs->size) - 1);
    if (col < 0) {
        log_warn("command '%.*s' on key '%.*s' failed: invalid column id",
                cmd->bstr.len, cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_get_ex);

        return;
    }

    val = bitset_get(bs, col);

    rsp->type = reply->type = ELEM_INT;
    reply->num = (int64_t)val;

    log_verb("command '%.*s' key '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
    INCR(process_metrics, bitmap_get_found);
}

void
cmd_bitmap_set(struct response *rsp, struct request *req, struct command *cmd)
{
    struct item *it;
    int32_t col;
    int16_t val;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bitset *bs;

    INCR(process_metrics, bitmap_set);

    it = cuckoo_get(key);
    if (it == NULL) {
        rsp->type = reply->type = ELEM_STR;
        reply->bstr = str2bstr(RSP_NOTFOUND);
        log_verb("command '%.*s' on key '%.*s' : key not found", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_set_notfound);

        return;
    }

    bs = it2bitset(it);

    col = _get_col(rsp, req, size2bit(bs->size) - 1);
    if (col < 0) {
        log_warn("command '%.*s' on key '%.*s' failed: invalid column id",
                cmd->bstr.len, cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_set_ex);

        return;
    }

    val = _get_bitval(rsp, req, (1 << bs->col_w) - 1);
    if (val < 0) {
        log_warn("command '%.*s' on key '%.*s' failed: invalid value",
                cmd->bstr.len, cmd->bstr.data, key->len, key->data);
        INCR(process_metrics, bitmap_set_ex);

        return;
    }
    bitset_set(bs, (uint16_t)col, val);

    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_OK);

    log_verb("command '%.*s' key '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
    INCR(process_metrics, bitmap_set_stored);
}
