#include "process.h"
#include "shared.h"

#include "data_structure/sarray/sarray.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>


static inline struct item *
_add_key(struct response *rsp, struct bstring *key)
{
    struct element *reply = (struct element *)array_get(rsp->token, 0);
    struct item *it;
    item_rstatus_e istatus;

    it = item_get(key);
    if (it != NULL) {
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, sarray_create_exist);

        return NULL;
    } else {
        /* TODO: figure out a TTL story here */
        istatus = item_reserve(&it, key, NULL, SARRAY_HEADER_SIZE, 0, INT32_MAX);
        if (istatus != ITEM_OK) {
            rsp->type = reply->type = ELEM_ERR;
            reply->bstr = str2bstr(RSP_ERR_STORAGE);
            INCR(process_metrics, sarray_create_ex);
            INCR(process_metrics, process_ex);
        } else {
            INCR(process_metrics, sarray_create_ok);
        }

        return it;
    }
}

/**
 * Attempt to extend an item by delta bytes. This is accomplished by first
 * checking if adding delta bytes to payload of it would require a larger item
 * to fit.
 *  - If no, then returns OK status without altering item.
 *  - If yes, then attempts to reserve an item that would be large enough. If
 *    this succeeds, then `it' is updated to the new item, and the payload of
 *    the old item is copied to the new one. If allocation fails, then a failure
 *    status is returned, and `it' remain unchanged.
 */
static inline item_rstatus_e
_realloc_key(struct item **it, const struct bstring *key, uint32_t delta)
{
    ASSERT(it != NULL && *it != NULL);

    if (!item_will_fit(*it, delta)) {
        /* must alloc new item, cannot fit in place */
        struct item *nit;
        item_rstatus_e istatus;

        /* carry over all applilcable item metadata */
        istatus = item_reserve(&nit, key, NULL, item_nval(*it) + delta,
                (*it)->olen, (*it)->expire_at);
        if (istatus != ITEM_OK) {
            return istatus;
        }
        /*copy item payload */
        cc_memcpy(nit->end, (*it)->end, item_npayload(*it));

        *it = nit;
        item_insert(nit, key);
    }

    ASSERT(item_will_fit(*it, delta));
    return ITEM_OK;
}

void
cmd_sarray_create(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    int64_t esize;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_create);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }
    log_verb("before esize");
    if (!req_get_int(&esize, req, SARRAY_ESIZE)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_ex);

        return;
    }
    log_verb("post parse");

    it = _add_key(rsp, key);
    if (it == NULL) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }

    /* initialize data structure */
    sarray_init((sarray_p)item_data(it), (uint32_t)esize);
    it->vlen = SARRAY_HEADER_SIZE;

    item_insert(it, key);

    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, sarray_create_ok);
}

void
cmd_sarray_delete(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_delete);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_delete_ex);

        return;
    }

    if (item_delete(key)) {
        compose_rsp_ok(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_delete_ok);
    } else {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_delete_notfound);
    }
}

void
cmd_sarray_len(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    uint32_t nentry;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_len);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_len_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_len_notfound);

        return;
    }

    nentry = sarray_nentry((sarray_p)item_data(it));

    rsp->type = reply->type = ELEM_INT;
    reply->num = (int64_t)nentry;
    log_verb("command '%.*s' '%.*s' succeeded, sarray length %u",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, nentry);
}

void
cmd_sarray_find(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    uint32_t idx;
    int64_t val;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_find);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_ex);

        return;
    }
    if (!req_get_int(&val, req, SARRAY_VAL)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_notfound);

        return;
    }

    status = sarray_index(&idx, (sarray_p)item_data(it), (uint32_t)val);
    switch (status) {
    case SARRAY_OK:
        rsp->type = reply->type = ELEM_INT;
        reply->num = (int64_t)idx;
        log_verb("command '%.*s' '%.*s' succeeded, value %"PRIu64" at index "
                PRIu32, cmd->bstr.len, cmd->bstr.data, key->len, key->data, val,
                idx);
        INCR(process_metrics, sarray_find_ok);

        break;
    case SARRAY_ENOTFOUND:
        compose_rsp_nil(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_notamember);

        break;
    case SARRAY_EINVALID:
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_ex);

        break;
    default:
        compose_rsp_server_err(rsp, reply, cmd, key);
        NOT_REACHED();
    }
}

void
cmd_sarray_get(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    int64_t idx;
    uint64_t val;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_get);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_get_ex);

        return;
    }
    if (!req_get_int(&idx, req, SARRAY_IDX)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_get_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_get_notfound);

        return;
    }

    status = sarray_value(&val, (sarray_p)item_data(it), (uint32_t)idx);
    switch (status) {
    case SARRAY_OK:
        rsp->type = reply->type = ELEM_INT;
        reply->num = (int64_t)val;
        log_verb("command '%.*s' '%.*s' succeeded, index %"PRIu32" has value "
                PRIu64, cmd->bstr.len, cmd->bstr.data, key->len, key->data, idx,
                val);
        INCR(process_metrics, sarray_get_ok);

        break;
    case SARRAY_EOOB:
        compose_rsp_oob(rsp, reply, cmd, key, idx);
        INCR(process_metrics, sarray_get_oob);

        break;
    default:
        compose_rsp_server_err(rsp, reply, cmd, key);
        NOT_REACHED();
    }
}

void
cmd_sarray_insert(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    uint32_t delta = 0;
    int64_t val;
    sarray_p sa;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_insert);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }
    if (!req_get_int(&val, req, SARRAY_VAL)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_notfound);

        return;
    }

    delta = sarray_esize((sarray_p)item_data(it));
    if (_realloc_key(&it, key, delta) != ITEM_OK) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }

    sa = (sarray_p)item_data(it);
    status = sarray_insert(sa, (uint64_t)val);

    if (status == SARRAY_EINVALID) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }
    if (status == SARRAY_EDUP) {
        compose_rsp_noop(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_noop);

        return;
    }

    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, sarray_insert_ok);
}

void
cmd_sarray_remove(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    int64_t val;
    sarray_p sa;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_insert);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_ex);

        return;
    }
    if (!req_get_int(&val, req, SARRAY_VAL)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_notfound);

        return;
    }

    sa = (sarray_p)item_data(it);
    status = sarray_remove(sa, val);

    switch (status) {
    case SARRAY_OK:
        /* TODO: should we try to "fit" to a smaller item here? */
        compose_rsp_ok(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_ok);

        break;
    case SARRAY_ENOTFOUND:
        compose_rsp_noop(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_noop);

        break;
    case SARRAY_EINVALID:
        /* client error, bad argument */
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_ex);

        break;
    default:
        compose_rsp_server_err(rsp, reply, cmd, key);
        NOT_REACHED();
    }
}

void
cmd_sarray_truncate(struct response *rsp, const struct request *req, const
        struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    int64_t cnt;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_truncate);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_truncate_ex);

        return;
    }
    if (!req_get_int(&cnt, req, SARRAY_IDX)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_truncate_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_truncate_notfound);

        return;
    }

    status = sarray_truncate((sarray_p)item_data(it), cnt);
    if (status != SARRAY_OK) {
        compose_rsp_server_err(rsp, reply, cmd, key);
    }

    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, sarray_truncate_ok);
}
