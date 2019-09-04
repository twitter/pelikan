#include "process.h"

#include "data_structure/ziplist/ziplist.h"
#include "protocol/data/resp_include.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>


/* TODO: find a better place for this utility */
static inline void
_elem2blob(struct blob *blob, const struct element *elem)
{
    ASSERT(blob != NULL);
    ASSERT(elem != NULL);
    if (elem->type == ELEM_INT) {
        blob->type = BLOB_TYPE_INT;
        blob->vint = (uint64_t)elem->num;
    } else {
        blob->type = BLOB_TYPE_STR;
        blob->vstr = elem->bstr;
    }
}

static inline struct bstring *
_get_key(const struct request *req)
{
    ASSERT(array_nelem(req->token) > LIST_KEY);
    struct element *key = (struct element *)array_get(req->token, LIST_KEY);
    return &key->bstr;
}

static inline struct element *
_get_val(const struct request *req)
{
    ASSERT(array_nelem(req->token) > LIST_VAL);
    return (struct element *)array_get(req->token, LIST_VAL);
}

static inline bool
_get_idx(int64_t *idx, const struct request *req)
{
    ASSERT(array_nelem(req->token) > LIST_IDX);
    ASSERT(idx != NULL);
    struct element *e = (struct element *)array_get(req->token, LIST_IDX);
    if (e->type != ELEM_INT) {
        return false;
    }
    *idx = e->num;
    return true;
}

static inline bool
_get_vidx(int64_t *vidx, const struct request *req)
{
    ASSERT(array_nelem(req->token) > LIST_VIDX);
    ASSERT(vidx != NULL);
    struct element *e = (struct element *)array_get(req->token, LIST_VIDX);
    if (e->type != ELEM_INT) {
        return false;
    }
    *vidx = e->num;
    return true;
}

static inline bool
_get_cnt(int64_t *cnt, const struct request *req)
{
    ASSERT(array_nelem(req->token) > LIST_CNT);
    ASSERT(cnt != NULL);
    struct element *e = (struct element *)array_get(req->token, LIST_CNT);
    if (e->type != ELEM_INT) {
        return false;
    }
    *cnt = e->num;
    return true;
}

static inline struct item *
_add_key(struct response *rsp, const struct bstring *key)
{
    struct element *reply = (struct element *)array_get(rsp->token, 0);
    struct item *it;
    item_rstatus_e istatus;

    it = item_get(key);
    if (it != NULL) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, list_create_exist);

        return NULL;
    } else {
        /* TODO: figure out a TTL story here */
        istatus = item_reserve(&it, key, NULL, ZIPLIST_HEADER_SIZE, 0, INT32_MAX);
        if (istatus != ITEM_OK) {
            rsp->type = reply->type = ELEM_ERR;
            reply->bstr = str2bstr(RSP_ERR_STORAGE);
            INCR(process_metrics, list_create_ex);
            INCR(process_metrics, process_ex);
        } else {
            INCR(process_metrics, list_create_stored);
        }

        return it;
    }
}

/**
 * Attempt to make item large enough to extend list by delta bytes. This is
 * accomplished by first checking if adding delta bytes to payload of it would
 * require a larger item to fit.
 *  - If no, then returns OK status without altering item.
 *  - If yes, then attempts to reserve an item that would be large enough. If
 *    this succeeds, then it and zl are updated to the new item and its payload
 *    respectively. If this fails, then a failure status is returned, and it
 *    and zl remain unchanged.
 */
static inline item_rstatus_e
_realloc_list_item(struct item **it, ziplist_p *zl, const struct bstring *key,
        uint32_t delta)
{
    ASSERT(it != NULL && *it != NULL);
    ASSERT(zl != NULL && *zl != NULL);
    ASSERT(key != NULL);

    if (!item_will_fit(*it, delta)) {
        /* must alloc new item, cannot fit in place */
        struct item *nit;
        struct bstring zl_str;
        item_rstatus_e istatus;

        zl_str.len = ziplist_size(*zl);
        zl_str.data = (char *)*zl;

        istatus = item_reserve(&nit, key, &zl_str, item_nval(*it) + delta,
                0, INT32_MAX);

        if (istatus != ITEM_OK) {
            return istatus;
        }

        *it = nit;
        *zl = (ziplist_p)item_data(nit);
        item_insert(nit, key);
    }

    ASSERT(item_will_fit(*it, delta));
    return ITEM_OK;
}

static inline void
_rsp_ok(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_OK);
    log_verb("command '%.*s' '%.*s' succeeded", cmd->bstr.len, cmd->bstr.data,
            key->len, key->data);
}

static inline void
_rsp_oob(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key, int64_t idx)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_OUTOFRANGE);
    log_verb("command '%.*s' '%.*s' has idx %lld out of bounds",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, idx);
}

static inline void
_rsp_notfound(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_STR;
    reply->bstr = str2bstr(RSP_NOTFOUND);
    log_verb("command '%.*s' '%.*s' miss, key not found",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
_rsp_client_err(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_ARG);
    INCR(process_metrics, process_client_ex);
    log_verb("command '%.*s' '%.*s' has invalid arg(s)",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

static inline void
_rsp_storage_err(struct response *rsp, struct element *reply,
        const struct command *cmd, const struct bstring *key)
{
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_STORAGE);
    INCR(process_metrics, process_server_ex);
    log_verb("command '%.*s' '%.*s' failed, unable to allocate storage",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}


void
cmd_list_create(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct item *it;
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);

    INCR(process_metrics, list_create);

    it = _add_key(rsp, key);
    if (it == NULL) {
        log_debug("command '%.*s' '%.*s' failed: cannot store", cmd->bstr.len,
                cmd->bstr.data, key->len, key->data);
        return;
    }

    /* initialize data structure */
    ziplist_reset((ziplist_p)item_data(it));
    it->vlen = ZIPLIST_HEADER_SIZE;

    /* link into index */
    item_insert(it, key);

    _rsp_ok(rsp, reply, cmd, key);
}

static void
_delete_list(struct element *reply, struct response *rsp, const struct bstring *key,
        const struct command *cmd)
{
    if (item_delete(key)) {
        _rsp_ok(rsp, reply, cmd, key);
        INCR(process_metrics, list_delete_deleted);
    } else {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_delete_notfound);
    }
}

static void
_delete_list_vals(struct element *reply, struct response *rsp, const struct bstring *key,
        const struct element *val, const struct command *cmd, int64_t cnt)
{
    struct item *it = item_get(key);
    ziplist_p zl;
    ziplist_rstatus_e status;
    struct blob vblob;
    uint32_t removed;

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_delete_notfound);
        return;
    }

    /* count == 0 means remove all */
    cnt = cnt == 0 ? INT64_MAX : cnt;

    zl = (ziplist_p)item_data(it);
    _elem2blob(&vblob, val);
    status = ziplist_remove_val(&removed, zl, &vblob, cnt);

    switch (status) {
    case ZIPLIST_OK:
        /* TODO: should we try to "fit" to a smaller item here? */
        rsp->type = reply->type = ELEM_INT;
        reply->num = removed;
        INCR(process_metrics, list_delete_deleted);
        log_verb("command '%.*s' '%.*s' succeeded, %u entries deleted",
                cmd->bstr.len, cmd->bstr.data, key->len, key->data, removed);
        break;
    case ZIPLIST_EINVALID:
        /* client error, bad argument */
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_ERR_ARG);
        INCR(process_metrics, process_client_ex);
        log_verb("command '%.*s' '%.*s' has invalid arg(s)",
                cmd->bstr.len, cmd->bstr.data, key->len, key->data);
        break;
    default:
        /* should never return ZIPLIST_ERROR, because
           zl and val should never be NULL */
        NOT_REACHED();
    }
}

void
cmd_list_delete(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    uint32_t narg = array_nelem(req->token);
    int64_t cnt;

    INCR(process_metrics, list_delete);

    /* client error from not enough args should be handled in parse phase */
    ASSERT(narg >= cmd->narg);

    switch (narg - 1) {
    case LIST_KEY:
        /* only key given, delete entire list */
        _delete_list(reply, rsp, key, cmd);
        break;
    case LIST_VAL:
        /* key + val given, remove all instances of val */
        /* TODO: verify that the behavior described above is what we want */
        _delete_list_vals(reply, rsp, key, _get_val(req), cmd, 0);
        break;
    case LIST_CNT:
        if (!_get_cnt(&cnt, req)) {
            _rsp_client_err(rsp, reply, cmd, key);
            return;
        }
        _delete_list_vals(reply, rsp, key, _get_val(req), cmd, cnt);
        break;
    default:
        /* client error from too many args should be handled in parse phase */
        NOT_REACHED();
    }
}

void
cmd_list_trim(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct item *it = item_get(key);
    ziplist_p zl;
    ziplist_rstatus_e status;
    int64_t idx, cnt;

    /* client error from wrong # args should be handled in parse phase */
    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, list_trim);

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_trim_notfound);
        return;
    }

    zl = (ziplist_p)item_data(it);

    if (!_get_idx(&idx, req)) {
        _rsp_client_err(rsp, reply, cmd, key);
        return;
    }

    if (!_get_cnt(&cnt, req)) {
        _rsp_client_err(rsp, reply, cmd, key);
        return;
    }

    status = ziplist_trim(zl, idx, cnt);

    if (status != ZIPLIST_OK) {
        /* other ziplist errs should not occur, since we
           have already done all of our input checking */
        ASSERT(status == ZIPLIST_EOOB);
        _rsp_oob(rsp, reply, cmd, key, idx);
        INCR(process_metrics, list_trim_oob);
        return;
    }

    /* TODO: should we try to "fit" to a smaller item here? */

    _rsp_ok(rsp, reply, cmd, key);
}

void
cmd_list_len(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct item *it = item_get(key);
    ziplist_p zl;
    uint32_t nentry;

    /* client error from wrong # args should be handled in parse phase */
    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, list_len);

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_len_notfound);
        return;
    }

    zl = (ziplist_p)item_data(it);
    nentry = ziplist_nentry(zl);

    rsp->type = reply->type = ELEM_INT;
    reply->num = (int64_t)nentry;
    log_verb("command '%.*s' '%.*s' succeeded, list length %u",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, nentry);
}

void
cmd_list_find(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    /* TODO: this command doesn't seem to have a counterpart in redis. let's
             re-evaluate whether or not we want to support this functionality
             at a later date. */
    INCR(process_metrics, list_find);
    struct element *reply = (struct element *)array_push(rsp->token);
    rsp->type = reply->type = ELEM_ERR;
    reply->bstr = str2bstr(RSP_ERR_NOSUPPORT);
}

void
cmd_list_get(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct item *it = item_get(key);
    ziplist_p zl;
    zipentry_p ze;
    ziplist_rstatus_e status;
    int64_t idx;
    struct blob val;

    /* client error from wrong # args should be handled in parse phase */
    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, list_get);

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_get_notfound);
        return;
    }

    zl = (ziplist_p)item_data(it);

    if (!_get_idx(&idx, req)) {
        _rsp_client_err(rsp, reply, cmd, key);
        return;
    }

    status = ziplist_locate(&ze, zl, idx);

    if (status != ZIPLIST_OK) {
        /* other error status should not happen, we have checked all our input */
        ASSERT(status == ZIPLIST_EOOB);
        _rsp_oob(rsp, reply, cmd, key, idx);
        INCR(process_metrics, list_get_oob);
        return;
    }

    status = zipentry_get(&val, ze);

    /* val should be valid if it was inserted properly */
    ASSERT(status == ZIPLIST_OK);

    switch (val.type) {
    case (BLOB_TYPE_INT):
        rsp->type = reply->type = ELEM_INT;
        reply->num = (int64_t)val.vint;
        break;
    case (BLOB_TYPE_STR):
        rsp->type = reply->type = ELEM_BULK;
        reply->bstr = val.vstr;
        break;
    default:
        NOT_REACHED();
    }

    log_verb("command '%.*s' '%.*s' succeeded",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data);
}

void
cmd_list_insert(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct item *it = item_get(key);
    ziplist_p zl;
    ziplist_rstatus_e status;
    struct blob vblob;
    int64_t idx;
    uint8_t ze_len;

    /* client error from wrong # args should be handled in parse phase */
    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, list_insert);

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_insert_notfound);
        return;
    }

    zl = (ziplist_p)item_data(it);
    _elem2blob(&vblob, _get_val(req));

    if (!_get_vidx(&idx, req)) {
        _rsp_client_err(rsp, reply, cmd, key);
        return;
    }

    /* pre-emptively check idx is in bounds, so we don't do extra work if not */
    if (idx >= ziplist_nentry(zl)) {
        _rsp_oob(rsp, reply, cmd, key, idx);
        INCR(process_metrics, list_insert_oob);
        return;
    }

    if (zipentry_size(&ze_len, &vblob) != ZIPLIST_OK) {
        /* val is invalid type or too long */
        _rsp_client_err(rsp, reply, cmd, key);
        return;
    }

    if (_realloc_list_item(&it, &zl, key, (uint32_t)ze_len) != ITEM_OK) {
        _rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, list_insert_ex);
        return;
    }

    ASSERT(item_will_fit(it, (uint32_t)ze_len));

    status = ziplist_insert(zl, &vblob, idx);

    /* any errs should not occur, given the input checking above */
    ASSERT(status == ZIPLIST_OK);

    _rsp_ok(rsp, reply, cmd, key);
}

void
cmd_list_push(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct bstring *key = _get_key(req);
    struct element *reply = (struct element *)array_push(rsp->token);
    struct item *it = item_get(key);
    uint32_t i, delta = 0;
    ziplist_p zl;
    ziplist_rstatus_e status;

    /* client error from wrong # args should be handled in parse phase */
    ASSERT(array_nelem(req->token) >= cmd->narg);

    INCR(process_metrics, list_push);

    if (it == NULL) {
        _rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, list_push_notfound);
        return;
    }

    zl = (ziplist_p)item_data(it);

    /* calculate additional length of ziplist after pushing all vals */
    for (i = LIST_VAL; i < array_nelem(req->token); ++i) {
        struct blob vblob;
        uint8_t ze_sz;
        _elem2blob(&vblob, array_get(req->token, i));

        if (zipentry_size(&ze_sz, &vblob) != ZIPLIST_OK) {
            /* val is invalid for list types */
            _rsp_client_err(rsp, reply, cmd, key);
            return;
        }

        delta += ze_sz;
    }

    if (_realloc_list_item(&it, &zl, key, delta) != ITEM_OK) {
        _rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, list_push_ex);
        return;
    }

    ASSERT(item_will_fit(it, delta));

    for (i = LIST_VAL; i < array_nelem(req->token); ++i) {
        struct blob vblob;
        _elem2blob(&vblob, array_get(req->token, i));
        status = ziplist_push(zl, &vblob);

        /* invalid val errs should have been taken care of above */
        ASSERT(status == ZIPLIST_OK);
    }

    _rsp_ok(rsp, reply, cmd, key);
}
