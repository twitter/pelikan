#include "process.h"
#include "shared.h"

#include "data_structure/smap/smap.h"
#include "storage/slab/item.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_bstring.h>
#include <cc_debug.h>
#include <cc_mm.h>

#define WATERMARK_SIZE (sizeof(uint32_t) * 2)  /* <low, high> entries in u32 */
/* TODO(yao): make MAX_NELEM configurable */
#define MAX_NELEM 255  /* max no. of elements to insert/remove per request */

static uint64_t ikeys[MAX_NELEM];
static struct bstring *vals[MAX_NELEM];
static struct bstring null_key = null_bstring;


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
cmd_smap_create(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    item_rstatus_e istatus;
    uint32_t ntoken;
    bool bounded;
    uint64_t isize, vsize, low, high;

    ntoken = array_nelem(req->token);
    ASSERT(ntoken >= cmd->narg);

    INCR(process_metrics, smap_create);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }
    if (!req_get_uint(&isize, req, SMAP_ISIZE)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }
    if (!req_get_uint(&vsize, req, SMAP_VSIZE)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }

    bounded = (cmd->nopt > 0);
    if (bounded && cmd->nopt != 2) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }

    /* get low & high watermarks */
    if (cmd->nopt > 0 && (!req_get_uint(&low, req, SMAP_WML) ||
                !req_get_uint(&high, req, SMAP_WMH))) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }

    it = item_get(key);
    if (it != NULL) { /* do not add key if exists */
        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_EXIST);
        INCR(process_metrics, smap_create_exist);

        return;
    }

    /* TODO: figure out a TTL story here */
    istatus = item_reserve(&it, key, NULL, SMAP_HEADER_SIZE,
            WATERMARK_SIZE * bounded, INT32_MAX);
    if (istatus != ITEM_OK) {
        compose_rsp_storage_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_create_ex);

        return;
    }

    /* initialize data structure */
    smap_init((smap_p)item_data(it), (uint16_t)isize, (uint16_t)vsize);
    it->vlen = SMAP_HEADER_SIZE;
    if (bounded) {
        _set_watermark((uint32_t *)item_optional(it), low, high);
    }

    item_insert(it, key);

    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, smap_create_ok);
}

void
cmd_smap_delete(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, smap_delete);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_delete_ex);

        return;
    }

    if (item_delete(key)) {
        compose_rsp_ok(rsp, reply, cmd, key);
        INCR(process_metrics, smap_delete_ok);
    } else {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_delete_notfound);
    }
}

void
cmd_smap_len(struct response *rsp, const struct request *req,
        const struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    uint32_t nentry;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, smap_len);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_len_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_len_notfound);

        return;
    }

    nentry = smap_nentry((smap_p)item_data(it));
    compose_rsp_numeric(rsp, reply, cmd, key, (int64_t)nentry);
}

void
cmd_smap_find(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    uint32_t idx;
    uint64_t ikey;
    smap_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, smap_find);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_find_ex);

        return;
    }
    if (!req_get_uint(&ikey, req, SMAP_IKEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_find_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_find_notfound);

        return;
    }

    status = smap_index(&idx, (smap_p)item_data(it), ikey);
    switch (status) {
    case SMAP_OK:
        rsp->type = reply->type = ELEM_INT;
        reply->num = (int64_t)idx;
        log_verb("command '%.*s' '%.*s' succeeded, ikey %"PRIu64" at index "
                PRIu32, cmd->bstr.len, cmd->bstr.data, key->len, key->data,
                ikey, idx);
        INCR(process_metrics, smap_find_ok);

        break;
    case SMAP_ENOTFOUND:
        compose_rsp_nil(rsp, reply, cmd, key);
        INCR(process_metrics, smap_find_notamember);

        break;
    case SMAP_EINVALID:
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_find_ex);

        break;
    default:
        compose_rsp_server_err(rsp, reply, cmd, key);
        NOT_REACHED();
    }
}

void
cmd_smap_get(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    int64_t idx = 0, cnt = 1;
    int64_t *ikey;
    uint32_t narg, nentry, nreturned = 0;
    int32_t incr;
    smap_rstatus_e status;

    narg = array_nelem(req->token);
    ASSERT(narg >= cmd->narg);

    INCR(process_metrics, smap_get);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_get_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_get_notfound);

        return;
    }

    nentry = smap_nentry((smap_p)item_data(it));

    if (narg > cmd->narg && !req_get_int(&idx, req, SMAP_IDX)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_get_ex);

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

    if (narg > cmd->narg + 1 && !req_get_int(&cnt, req, SMAP_ICNT)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_get_ex);

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
        reply->type = ELEM_INT;
        ikey = &(reply->num);
        reply = (struct element *)array_push(rsp->token);
        reply->type = ELEM_BULK;
        status = smap_keyval((uint64_t *)ikey, &(reply->bstr), (smap_p)item_data(it),
                (uint32_t)idx);
        ASSERT(status == SMAP_OK);
        reply = (struct element *)array_push(rsp->token);
    }
    array_pop(rsp->token);

    INCR(process_metrics, smap_get_ok);
    log_verb("command '%.*s' '%.*s' succeeded, returning %"PRIu32" elements",
            cmd->bstr.len, cmd->bstr.data, key->len, key->data, array_nelem(rsp->token));
}

void
cmd_smap_insert(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    uint16_t vsize;
    uint32_t nelem = 0, ntotal, esize;
    int64_t delta, wml, wmh, nentry, ninserted = 0;
    uint64_t ikey;
    smap_p sm;
    smap_rstatus_e status;

    ASSERT(array_nelem(req->token) >= cmd->narg);

    INCR(process_metrics, smap_insert);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_insert_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_insert_notfound);

        return;
    }

    ntotal = (array_nelem(req->token) - SMAP_IKEY) + 1;
    if (ntotal & 0x1) { /* odd number of tokens left */
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_insert_ex);

        return;
    } else {
        ntotal = ntotal >> 1;
    }

    sm = (smap_p)item_data(it);
    vsize = smap_vsize(sm);
    /* parse and store all values to be inserted in array vals */
    for (uint32_t i = SMAP_IKEY; nelem < ntotal; i += 2, ++nelem) {
        if (!req_get_uint(&ikey, req, i)) {
            log_debug("the integer key at offset %"PRIu32" is invalid", i);
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_insert_ex);

            return;
        }

        ikeys[nelem] = ikey;
        req_get_bstr(&vals[nelem], req, i + 1);
        if (vals[nelem]->len != (uint32_t)vsize) {
            log_debug("value size %"PRIu32" at offset %"PRIu32" is "
                    "incompatible with current SMap config of %"PRIu16,
                    vals[nelem]->len, i, vsize);
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_insert_ex);

            return;
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
    esize = smap_esize(sm);
    delta = esize * ntotal;

    /**
     * Attempt to extend an item by delta bytes. This is accomplished by first
     * checking if adding delta bytes to payload of it would require a larger item
     * to fit.
     */
    if (!item_will_fit(it, delta)) {
        /* must alloc new item, cannot fit in place */
        struct item *nit;

        /* carry over all applicable item metadata */
        if (item_reserve(&nit, key, NULL, item_nval(it) + delta, it->olen,
                    it->expire_at) != ITEM_OK) {
            log_debug("reallocate item for key '%.*s' failed", key->len,
                    key->data);
            compose_rsp_storage_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_insert_ex);

            return;
        }

        log_verb("successfully resized item for key '%.*s' to allow delta of %"
                PRIu32" bytes", key->len, key->data, delta);

        /*copy item payload */
        /* NOTE(yao): we are double copying the key portion here */
        cc_memcpy(nit->end, it->end, item_npayload(it));
        nit->vlen = it->vlen;
        it = nit;
        item_insert(nit, key);
    }

    sm = (smap_p)item_data(it); /* item might have changed */
    for (uint32_t i = 0; i < ntotal; ++i) {
        status = smap_insert(sm, ikeys[i], vals[i]);
        if (status == SMAP_EINVALID) {
            log_debug("value %"PRIu32" out of %"PRIu32" is invalid", i, ntotal);
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_insert_ex);
            return;
        }

        if (status == SMAP_EDUP) {
            INCR(process_metrics, smap_insert_noop);
        } else {
            INCR(process_metrics, smap_insert_ok);
            ninserted++;
            it->vlen += esize;
        }
    }

    if (it->olen > 0) {
        wml = _watermark_low((uint32_t *)item_optional(it));
        wmh = _watermark_high((uint32_t *)item_optional(it));
        nentry = smap_nentry(sm);
        if (nentry > wmh) {
            log_verb("truncating '%.*s' from %"PRIu32" down to %"PRIu32" elements",
                key->len, key->data, nentry, wml);
            INCR(process_metrics, smap_insert_trim);
            smap_truncate(sm, nentry - wml);
            it->vlen -= esize * (nentry - wml);
        }
    }

    compose_rsp_numeric(rsp, reply, cmd, key, ninserted);
}

void
cmd_smap_remove(struct response *rsp, const struct request *req, const struct
        command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    uint32_t nelem = 0, nremoved = 0, esize;
    int64_t ikey;
    smap_p sm;
    smap_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, smap_insert);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_remove_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_remove_notfound);

        return;
    }

    /* parse and store all ikeys to be removed in array ikeys */
    for (uint32_t i = SMAP_IKEY; i < array_nelem(req->token); ++i) {
        if (!req_get_int(&ikey, req, i)) {
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_remove_ex);

            return;
        }

        ikeys[nelem] = (uint64_t)ikey;
        nelem++;
    }

    /* TODO: should we try to "fit" to a smaller item here? */
    sm = (smap_p)item_data(it);
    esize = smap_esize(sm);
    for (uint32_t i = 0; i < nelem; ++i) {
        status = smap_remove(sm, ikeys[i]);
        switch (status) {
        case SMAP_OK:
            nremoved++;
            it->vlen -= esize;
            INCR(process_metrics, smap_remove_ok);

            break;
        case SMAP_ENOTFOUND:
            compose_rsp_noop(rsp, reply, cmd, key);
            INCR(process_metrics, smap_remove_noop);

            break;
        case SMAP_EINVALID:
            /* client error, bad argument */
            log_debug("value %"PRIu32" out of %"PRIu32" is invalid", i, nelem);
            compose_rsp_client_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_remove_ex);

            return;
        default:
            compose_rsp_server_err(rsp, reply, cmd, key);
            INCR(process_metrics, smap_remove_ex);
            NOT_REACHED();
            return;
        }
    }

    compose_rsp_numeric(rsp, reply, cmd, key, (int64_t)nremoved);
}

void
cmd_smap_truncate(struct response *rsp, const struct request *req, const
        struct command *cmd)
{
    struct element *reply = (struct element *)array_push(rsp->token);
    struct bstring *key = &null_key;
    struct item *it;
    int64_t cnt;
    smap_p sm;
    smap_rstatus_e status;

    ASSERT(array_nelem(req->token) == cmd->narg);

    INCR(process_metrics, smap_truncate);

    if (!req_get_bstr(&key, req, SMAP_KEY)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_truncate_ex);

        return;
    }
    if (!req_get_int(&cnt, req, SMAP_IDX)) {
        compose_rsp_client_err(rsp, reply, cmd, key);
        INCR(process_metrics, smap_truncate_ex);

        return;
    }

    it = item_get(key);
    if (it == NULL) {
        compose_rsp_notfound(rsp, reply, cmd, key);
        INCR(process_metrics, smap_truncate_notfound);

        return;
    }

    sm = (smap_p)item_data(it);
    status = smap_truncate(sm, cnt);
    if (status != SMAP_OK) {
        compose_rsp_server_err(rsp, reply, cmd, key);
    }

    it->vlen = SMAP_HEADER_SIZE + smap_esize(sm) * smap_nentry(sm);
    compose_rsp_ok(rsp, reply, cmd, key);
    INCR(process_metrics, smap_truncate_ok);
}
