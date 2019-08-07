#include "process.h"
#include "shared.h"

#include "data_structure/sarray/sarray.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

/* TODO(yao): make MAX_NVAL configurable */
#define MAX_NVAL 255  /* max no. of values to insert/remove in one request */
static uint64_t vals[MAX_NVAL];


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
    compose_rsp_numeric(rsp, reply, cmd, key, (int64_t)nentry);
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
    int64_t idx = 0, cnt = 1;
    uint64_t val;
    uint32_t narg, nentry, nreturned = 0;
    int32_t incr;
    sarray_rstatus_e status;

    narg = array_nelem(req->token);
    ASSERT(narg >= cmd->narg);

    INCR(process_metrics, sarray_get);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
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

    nentry = sarray_nentry((sarray_p)item_data(it));

    if (narg > cmd->narg && !req_get_int(&idx, req, SARRAY_IDX)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_get_ex);

        return;
    }

    if (idx < 0) {
        idx += nentry;
        if (idx < 0) {
            idx = 0;
        }
    } else {
        if (idx > nentry) {
            idx = nentry;
        }
    }

    if (narg > cmd->narg + 1 && !req_get_int(&cnt, req, SARRAY_ICNT)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_get_ex);

        return;
    }
    /* cnt < 0 means return in reverse order */
    if (cnt > 0) {
        incr = 1;
        nreturned = MIN(nentry - idx, cnt);
    } else {
        incr = -1;
        nreturned = MIN(idx + 1, -cnt);
    }

    /* write the array header */
    rsp->type = ELEM_ARRAY;
    for (; nreturned > 0; nreturned--, idx += incr) {
        status = sarray_value(&val, (sarray_p)item_data(it), (uint32_t)idx);
        ASSERT(status == SARRAY_OK);
        reply->type = ELEM_INT;
        reply->num = val;
        reply = (struct element *)array_push(rsp->token);
    }
    array_pop(rsp->token);

    INCR(process_metrics, sarray_get_ok);
    log_verb("command '%.*s' '%.*s' succeeded, returning %"PRIu32" elements",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, array_nelem(rsp->token));
}

void
cmd_sarray_insert(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    uint32_t nval = 0, ninserted = 0, delta;
    int64_t val;
    sarray_p sa;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) >= cmd->narg);

    INCR(process_metrics, sarray_insert);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
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

    /* parse and store all values to be inserted in array vals */
    for (uint32_t i = SARRAY_VAL; i < array_nelem(req->token); ++i, ++nval) {
        if (!req_get_int(&val, req, i)) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_insert_ex);

            return;
        } else {
            vals[nval] = (uint64_t)val;
        }
    }

    delta = sarray_esize((sarray_p)item_data(it)) * nval;
    if (_realloc_key(&it, key, delta) != ITEM_OK) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }

    sa = (sarray_p)item_data(it);
    for (uint32_t i = 0; i < nval; ++i) {
        status = sarray_insert(sa, vals[i]);
        if (status == SARRAY_EINVALID) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_insert_ex);
            return;
        }

        if (status == SARRAY_EDUP) {
            compose_rsp_noop(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_insert_noop);
        } else {
            INCR(process_metrics, sarray_insert_ok);
            ninserted++;
        }
    }

    compose_rsp_numeric(rsp, reply, cmd, key, (int64_t)ninserted);
}

void
cmd_sarray_remove(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key;
    struct item *it;
    uint32_t nval = 0, nremoved = 0;
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

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_remove_notfound);

        return;
    }

    /* parse and store all values to be inserted in array vals */
    for (uint32_t i = SARRAY_VAL; i < array_nelem(req->token); ++i) {
        if (!req_get_int(&val, req, i)) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_remove_ex);

            return;
        } else {
            vals[nval] = (uint64_t)val;
            nval++;
        }
    }
    /* TODO: should we try to "fit" to a smaller item here? */

    sa = (sarray_p)item_data(it);
    for (uint32_t i = 0; i < nval; ++i) {
        status = sarray_remove(sa, vals[i]);
        switch (status) {
        case SARRAY_OK:
            nremoved++;
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

            return;
        default:
            compose_rsp_server_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_remove_ex);
            NOT_REACHED();
            return;
        }
    }

    compose_rsp_numeric(rsp, reply, cmd, key, (int64_t)nremoved);
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
