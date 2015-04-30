#include <twemcache/bb_process.h>

#include <protocol/memcache/bb_codec.h>
#include <storage/slab/bb_item.h>

#include <cc_array.h>
#include <cc_log.h>

static void
process_bstring_data(struct bstring *val, struct item *it)
{
    ASSERT(val != NULL && it != NULL);

    val->len = it->vlen;
    val->data = (uint8_t *)item_data(it);
}

static rstatus_t
process_get_key(struct buf *buf, struct bstring *key)
{
    rstatus_t status = CC_OK;
    struct item *it;
    struct bstring val;

    log_verb("get key at %p, rsp buf at %p", key, buf);

    it = item_get(key);
    if (it != NULL) {
        log_verb("found key at item %p", it);
        process_bstring_data(&val, it);

        status = compose_rsp_keyval(buf, key, &val, item_flag(it), 0);
    } else {
        log_verb("item with key at %p not found", key);
    }

    return status;
}

static rstatus_t
process_get(struct request *req, struct buf *buf)
{
    rstatus_t status;
    struct bstring *key;
    uint32_t i;

    log_verb("processing get req %p with rsp buf at %p", req, buf);

    for (i = 0; i < req->keys->nelem; ++i) {
        key = array_get_idx(req->keys, i);
        status = process_get_key(buf, key);
        if (status != CC_OK) {
            return status;
        }
    }
    status = compose_rsp_msg(buf, RSP_END, false);

    return status;
}

static rstatus_t
process_gets(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_delete(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_set(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    item_rstatus_t i_status;
    rel_time_t exptime;
    struct bstring *key;

    log_verb("processing set req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    exptime = time_reltime(req->expiry);

    i_status = item_set(key, &(req->vstr), exptime);

    if (i_status == ITEM_OK) {
        status = compose_rsp_msg(buf, RSP_STORED, req->noreply);
    } else if (i_status == ITEM_EOVERSIZED) {
        /* set oversized */
        status = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    } else {
        /* Could not allocate memory */
        status = compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    }

    return status;
}

static rstatus_t
process_add(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_replace(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_cas(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_incr(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_decr(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_stats(struct request *req, struct buf *buf)
{
    /* not implemented yet */
    return CC_OK;
}

rstatus_t
process_request(struct request *req, struct buf *buf)
{
    log_verb("processing req %p, rsp buf at %p", req, buf);

    switch (req->verb) {
    case REQ_GET:
        return process_get(req, buf);

    case REQ_GETS:
        return process_gets(req, buf);

    case REQ_DELETE:
        return process_delete(req, buf);

    case REQ_SET:
        return process_set(req, buf);

    case REQ_ADD:
        return process_add(req, buf);

    case REQ_REPLACE:
        return process_replace(req, buf);

    case REQ_CAS:
        return process_cas(req, buf);

    case REQ_INCR:
        return process_incr(req, buf);

    case REQ_DECR:
        return process_decr(req, buf);

    case REQ_STATS:
        return process_stats(req, buf);

    case REQ_QUIT:
        return CC_ERDHUP;

    default:
        NOT_REACHED();
        break;
    }

    return CC_OK;
}
