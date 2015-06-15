#include <twemcache/bb_process.h>

#include <protocol/memcache/bb_codec.h>
#include <storage/slab/bb_item.h>
#include <twemcache/bb_stats.h>
#include <util/bb_procinfo.h>

#include <cc_array.h>
#include <cc_log.h>

#include <stdbool.h>
#include <stdio.h>

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

    if ((it = item_get(key))) {
        /* item found */
        log_verb("found key at item %p", it);
        process_bstring_data(&val, it);

        status = compose_rsp_keyval(buf, key, &val, item_flag(it), 0);
    } else {
        /* item not found */
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
process_gets_key(struct buf *buf, struct bstring *key)
{
    rstatus_t status = CC_OK;
    struct item *it;
    struct bstring val;

    log_verb("gets key at %p, rsp buf at %p", key, buf);

    if ((it = item_get(key))) {
        /* item found */
        log_verb("found key at item %p", it);
        process_bstring_data(&val, it);

        status = compose_rsp_keyval(buf, key, &val, item_flag(it), item_get_cas(it));
    } else {
        /* item not found */
        log_verb("item with key at %p not found", key);
    }

    return status;
}

static rstatus_t
process_gets(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    struct bstring *key;
    uint32_t i;

    log_verb("processing gets req %p, rsp buf at %p", req, buf);

    for (i = 0; i < req->keys->nelem; ++i) {
        key = array_get_idx(req->keys, i);
        status = process_gets_key(buf, key);
        if (status != CC_OK) {
            return status;
        }
    }
    status = compose_rsp_msg(buf, RSP_END, false);

    /* not implemented yet */
    return CC_OK;
}

static rstatus_t
process_delete(struct request *req, struct buf *buf)
{
    struct bstring *key;

    key = array_get_idx(req->keys, 0);

    if (item_delete(key) == ITEM_OK) {
        /* item successfully deleted */
        return compose_rsp_msg(buf, RSP_DELETED, req->noreply);
    }

    /* no item with that key */
    return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
}

static rstatus_t
process_set_key(struct request *req, struct bstring *key, struct buf *buf)
{
    item_rstatus_t i_status;
    rel_time_t exptime;

    exptime = time_reltime(req->expiry);

    i_status = item_set(key, &(req->vstr), exptime);

    switch (i_status) {
    case ITEM_OK:
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_EOVERSIZED:
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
    }

    return CC_ERROR;
}

static rstatus_t
process_set(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rstatus_t ret;

    log_verb("processing set req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    ret = process_set_key(req, key, buf);

    return ret;
}

static rstatus_t
process_add(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rstatus_t ret;

    log_verb("processing add req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if (item_get(key)) {
        /* key already exists, do not set */
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    } else {
        /* key does not exist, set */
        ret = process_set_key(req, key, buf);
    }

    return ret;
}

static rstatus_t
process_replace(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rstatus_t ret;

    log_verb("processing replace req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if (item_get(key)) {
        /* key exists, perform replace */
        ret = process_set_key(req, key, buf);
    } else {
        /* key does not exist, do not set */
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    }

    return ret;
}

static rstatus_t
process_cas(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rel_time_t exptime;

    log_verb("processing cas req %p, rsp buf at %p", req, buf);

    exptime = time_reltime(req->expiry);
    key = array_get_idx(req->keys, 0);

    switch (item_cas(key, &(req->vstr), exptime, req->cas)) {
    case ITEM_OK:
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_ENOTFOUND:
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    case ITEM_EOTHER:
        return compose_rsp_msg(buf, RSP_EXISTS, req->noreply);
    case ITEM_EOVERSIZED:
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
        break;
    }

    return CC_ERROR;
}

static rstatus_t
process_delta(struct request *req, struct buf *buf, bool incr)
{
    struct bstring *key, val_str, new_val_str;
    struct item *it;
    item_rstatus_t i_status;
    uint64_t val;
    char new_val_data[CC_UINT64_MAXLEN + 1];

    key = array_get_idx(req->keys, 0);

    if ((it = item_get(key))) {
        if (it->vtype == V_INT) {
            /* item val is int */

            /* get uint64_t version of value */
            val_str.len = it->vlen;
            val_str.data = (uint8_t *)item_data(it);
            bstring_atou64(&val, &val_str);

            /* apply delta */
            if (incr) {
                val += req->delta;
            } else {
                val -= req->delta;
            }

            /* prepare bstring for item_update */
            new_val_str.len = sprintf(new_val_data, "%llu", val);
            new_val_str.data = (uint8_t *)new_val_data;

            i_status = item_update(it, &new_val_str);

            if (i_status == CC_OK) {
                /* delta successful */
                return compose_rsp_uint64(buf, val, req->noreply);
            } else if (i_status == ITEM_EOVERSIZED) {
                /* oversized */
                log_warn("int with delta would be oversized (check the cache config, ints should not be causing oversize errors)");
                return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
            } else {
                NOT_REACHED();
                return CC_ERROR;
            }
        } else {
            /* non integer value */
            log_warn("value is not int, cannot apply delta on key %.*s val %.*s",
                     key->len, key->data, it->vlen, item_data(it));
            return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }
    } else {
        /* item not found */
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }
}

static rstatus_t
process_incr(struct request *req, struct buf *buf)
{
    rstatus_t ret;

    log_verb("processing incr req %p, rsp buf at %p", req, buf);
    ret = process_delta(req, buf, true);

    return ret;
}

static rstatus_t
process_decr(struct request *req, struct buf *buf)
{
    rstatus_t ret;

    log_verb("processing decr req %p, rsp buf at %p", req, buf);
    ret = process_delta(req, buf, false);

    return ret;
}

static rstatus_t
process_annex(struct request *req, struct buf *buf, bool append)
{
    struct bstring *key;
    item_rstatus_t i_status;

    log_verb("processing annex req %p, append %u, rsp buf at %p", req, append, buf);

    key = array_get_idx(req->keys, 0);
    i_status = item_annex(key, &(req->vstr), append);

    switch (i_status) {
    case ITEM_OK:
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_ENOTFOUND:
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    case ITEM_EOVERSIZED:
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
        break;
    }

    return CC_ERROR;
}

static rstatus_t
process_append(struct request *req, struct buf *buf)
{
    rstatus_t ret;

    log_verb("processing append req %p, rsp buf at %p", req, buf);
    ret = process_annex(req, buf, true);

    return ret;
}

static rstatus_t process_prepend(struct request *req, struct buf *buf)
{
    rstatus_t ret;

    log_verb("processing prepend req %p, rsp buf at %p", req, buf);
    ret = process_annex(req, buf, false);

    return ret;
}

static rstatus_t
process_stats(struct request *req, struct buf *buf)
{
    procinfo_update();
    return compose_rsp_stats(buf, (struct metric *)&glob_stats,
                             METRIC_CARDINALITY(glob_stats));
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

    case REQ_APPEND:
        return process_append(req, buf);

    case REQ_PREPEND:
        return process_prepend(req, buf);

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
