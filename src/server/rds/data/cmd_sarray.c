#include "process.h"
#include "shared.h"

#include "data_structure/sarray/sarray.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

#define WATERMARK_SIZE (sizeof(uint32_t) * 2)  /* <low, high> entries in u32 */
/* TODO(yao): make MAX_NVAL configurable */
#define MAX_NVAL 255  /* max no. of values to insert/remove in one request */

static uint64_t vals[MAX_NVAL];
static struct bstring null_key = null_bstring;

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
            log_debug("reallocate item for key '%.*s' failed: %d", key->len,
                    key->data, istatus);
            return istatus;
        }

        log_verb("successfully reallocated item for key '%.*s'", key->len,
                key->data);
        /*copy item payload */
        cc_memcpy(nit->end, (*it)->end, item_npayload(*it));

        *it = nit;
        item_insert(nit, key);
    }

    ASSERT(item_will_fit(*it, delta));
    return ITEM_OK;
}

static inline uint32_t
_watermark_low(uint32_t *opt)
{
    return *opt;
}

static inline uint32_t
_watermark_high(uint32_t *opt)
{
    return *(opt + 1);
}

static inline void
_set_watermark(uint32_t *opt, uint32_t low, uint32_t high)
{
    *opt = low;
    ++opt;
    *opt = high;
}

void
cmd_sarray_create(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    item_rstatus_e istatus;
    uint32_t ntoken;
    bool bounded;
    uint64_t esize, low, high;

    ntoken = array_nelem(req->token);
    ASSERT(ntoken >= cmd->narg);

    INCR(process_metrics, sarray_create);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }
    if (!req_get_uint(&esize, req, SARRAY_ESIZE)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }

    bounded = (cmd->nopt > 0);
    if (bounded && cmd->nopt != 2) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }

    /* get low & high watermarks */
    if (cmd->nopt > 0 && (!req_get_uint(&low, req, SARRAY_WML) ||
                !req_get_uint(&high, req, SARRAY_WMH))) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }

    it = item_get(key);
    if (it != NULL) { /* do not add key if exists */
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, sarray_create_exist);

        return;
    }

    /* TODO: figure out a TTL story here */
    istatus = item_reserve(&it, key, NULL, SARRAY_HEADER_SIZE,
            WATERMARK_SIZE * bounded, INT32_MAX);
    if (istatus != ITEM_OK) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_create_ex);

        return;
    }

    /* initialize data structure */
    sarray_init((sarray_p)item_data(it), (uint32_t)esize);
    it->vlen = SARRAY_HEADER_SIZE;
    if (bounded) {
        _set_watermark((uint32_t *)item_optional(it), low, high);
    }

    item_insert(it, key);

    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, sarray_create_ok);
}

void
cmd_sarray_delete(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;

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
    struct bstring *key = &null_key;
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
    struct bstring *key = &null_key;
    struct item *it;
    uint32_t idx;
    uint64_t val;
    sarray_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, sarray_find);

    if (!req_get_bstr(&key, req, SARRAY_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_find_ex);

        return;
    }
    if (!req_get_uint(&val, req, SARRAY_VAL)) {
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
    struct bstring *key = &null_key;
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
    struct bstring *key = &null_key;
    struct item *it;
    uint32_t nval = 0, esize;
    int64_t delta, wml, wmh, nentry, ninserted = 0;
    uint64_t val;
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
        if (!req_get_uint(&val, req, i)) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_insert_ex);

            return;
        } else {
            vals[nval] = val;
        }
    }

    /* we always insert everything before trying to truncate down an array
     * that is too long. The reason for that is because inserting is the
     * only way of ensuring the new values are properly sorted in the array,
     * and the truncation (designed to be from the left for now) is indeed
     * removing the lowest values. The downside is we may trigger an extra
     * realloc of the key and assign it more memory than the final size may
     * require.
     *
     * Example: if item can host at most an array of 10 elements, and we
     * create an array with watermarks [6, 8], inserting 6 elements into
     * an array of 4 elements will result in the array having 10 elements
     * before being trimmed back. So the following logic will try to allocate
     * (and keep) memory for 10 elements.
     *
     * However, this seems acceptable, mostly because we assume insert batch
     * size is relatively small compared to watermark settings in most cases,
     * and therefore users can configure their watermarks and control their
     * batch sizes to ensure insertion at maximum array size stays within a
     * single slabclass.
     */
    sa = (sarray_p)item_data(it);
    esize = sarray_esize(sa);
    delta = esize * nval;

    if (_realloc_key(&it, key, delta) != ITEM_OK) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, sarray_insert_ex);

        return;
    }

    sa = (sarray_p)item_data(it); /* item might have changed */
    for (uint32_t i = 0; i < nval; ++i) {
        status = sarray_insert(sa, vals[i]);
        if (status == SARRAY_EINVALID) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, sarray_insert_ex);
            return;
        }

        if (status == SARRAY_EDUP) {
            INCR(process_metrics, sarray_insert_noop);
        } else {
            INCR(process_metrics, sarray_insert_ok);
            ninserted++;
        }
    }

    if (it->olen > 0) {
        wml = _watermark_low((uint32_t *)item_optional(it));
        wmh = _watermark_high((uint32_t *)item_optional(it));
        nentry = sarray_nentry(sa);
        if (nentry > wmh) {
            log_verb("truncating '%.*s' from %"PRIu32" down to %"PRIu32" elements",
                key->len, key->data, nentry, wml);
            sarray_truncate(sa, nentry - wml);
        }
    }

    compose_rsp_numeric(rsp, reply, cmd, key, ninserted);
}

void
cmd_sarray_remove(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
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
    struct bstring *key = &null_key;
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
