#include <twemcache/process.h>

#include <protocol/memcache/codec.h>
#include <storage/slab/item.h>
#include <twemcache/stats.h>
#include <util/procinfo.h>

#include <cc_array.h>
#include <cc_log.h>

#include <stdbool.h>
#include <stdio.h>

#define TWEMCACHE_PROCESS_MODULE_NAME "twemcache::process"

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;

void
process_setup(process_metrics_st *metrics)
{
    log_info("set up the %s module", TWEMCACHE_PROCESS_MODULE_NAME);

    process_metrics = metrics;
    PROCESS_METRIC_INIT(process_metrics);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 TWEMCACHE_PROCESS_MODULE_NAME);
    }

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", TWEMCACHE_PROCESS_MODULE_NAME);

    if (!process_init) {
        log_warn("%s has never been setup", TWEMCACHE_PROCESS_MODULE_NAME);
    }

    process_metrics = NULL;
    process_init = false;
}

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
    INCR(process_metrics, cmd_get_key);

    if ((it = item_get(key))) {
        /* item found */
        log_verb("found key at item %p", it);
        INCR(process_metrics, cmd_get_key_hit);

        process_bstring_data(&val, it);

        status = compose_rsp_keyval(buf, key, &val, item_flag(it), 0);
    } else {
        /* item not found */
        log_verb("item with key at %p not found", key);
        INCR(process_metrics, cmd_get_key_miss);
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
    INCR(process_metrics, cmd_gets_key);

    if ((it = item_get(key))) {
        /* item found */
        log_verb("found key at item %p", it);
        INCR(process_metrics, cmd_gets_key_hit);
        process_bstring_data(&val, it);

        status = compose_rsp_keyval(buf, key, &val, item_flag(it), item_get_cas(it));
    } else {
        /* item not found */
        log_verb("item with key at %p not found", key);
        INCR(process_metrics, cmd_gets_key_miss);
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
        INCR(process_metrics, cmd_delete_deleted);
        return compose_rsp_msg(buf, RSP_DELETED, req->noreply);
    }

    /* no item with that key */
    INCR(process_metrics, cmd_delete_notfound);
    return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
}

static item_rstatus_t
process_set_key(struct request *req, struct bstring *key, struct buf *buf)
{
    rel_time_t exptime;

    exptime = time_reltime(req->expiry);

    return item_set(key, &(req->vstr), exptime);
}

static rstatus_t
process_set_rsp(struct request *req, struct buf *buf, item_rstatus_t i_status)
{
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
    item_rstatus_t i_status;

    log_verb("processing set req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    i_status = process_set_key(req, key, buf);

    if (i_status == ITEM_OK) {
        INCR(process_metrics, cmd_set_stored);
    } else {
        INCR(process_metrics, cmd_set_ex);
    }

    return process_set_rsp(req, buf, i_status);
}

static rstatus_t
process_add(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rstatus_t ret;
    item_rstatus_t i_status;

    log_verb("processing add req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if (item_get(key)) {
        /* key already exists, do not set */
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
        INCR(process_metrics, cmd_add_notstored);
    } else {
        /* key does not exist, set */
        i_status = process_set_key(req, key, buf);

        if (i_status == ITEM_OK) {
            INCR(process_metrics, cmd_add_stored);
        } else {
            INCR(process_metrics, cmd_add_ex);
        }

        ret = process_set_rsp(req, buf, i_status);
    }

    return ret;
}

static rstatus_t
process_replace(struct request *req, struct buf *buf)
{
    struct bstring *key;
    rstatus_t ret;
    item_rstatus_t i_status;

    log_verb("processing replace req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if (item_get(key)) {
        /* key exists, perform replace */
        i_status = process_set_key(req, key, buf);

        if (i_status == ITEM_OK) {
            INCR(process_metrics, cmd_replace_stored);
        } else {
            INCR(process_metrics, cmd_replace_ex);
        }

        ret = process_set_rsp(req, buf, i_status);
    } else {
        /* key does not exist, do not set */
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
        INCR(process_metrics, cmd_replace_notstored);
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
        INCR(process_metrics, cmd_cas_stored);
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_ENOTFOUND:
        INCR(process_metrics, cmd_cas_notfound);
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    case ITEM_EOTHER:
        INCR(process_metrics, cmd_cas_exists);
        return compose_rsp_msg(buf, RSP_EXISTS, req->noreply);
    case ITEM_EOVERSIZED:
        INCR(process_metrics, cmd_cas_ex);
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        INCR(process_metrics, cmd_cas_ex);
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
        break;
    }

    return CC_ERROR;
}

/* get integer value of it */
static inline uint64_t
_retrieve_item_val_u64(struct item *it)
{
    struct bstring val_str;
    uint64_t val;

    ASSERT(it->vtype == V_INT);

    val_str.len = it->vlen;
    val_str.data = (uint8_t *)item_data(it);

    bstring_atou64(&val, &val_str);

    return val;
}

/* update item with integer value */
static inline void
_store_item_val_u64(struct item *it, uint64_t val)
{
    struct bstring val_str;
    char val_data[CC_UINT64_MAXLEN + 1];
    item_rstatus_t i_status;

    val_str.data = (uint8_t *)val_data;
    val_str.len = sprintf((char *)val_str.data, "%llu", val);

    i_status = item_update(it, &val_str);

    /* If this assertion fails, item/slab is incorrectly configured */
    ASSERT(i_status == ITEM_OK);
}

static rstatus_t
process_incr(struct request *req, struct buf *buf)
{
    struct bstring *key;
    struct item *it;
    uint64_t val;

    log_verb("processing incr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if ((it = item_get(key))) {
        if (it->vtype == V_INT) {
            val = _retrieve_item_val_u64(it);

            val += req->delta;

            _store_item_val_u64(it, val);
            INCR(process_metrics, cmd_incr_stored);
            return compose_rsp_uint64(buf, val, req->noreply);
        }
        /* non integer value */
        log_verb("incr on key %.*s with non integer value", key->len, key->data);
        INCR(process_metrics, cmd_incr_ex);
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    }

    /* item not found */
    INCR(process_metrics, cmd_incr_notfound);
    return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
}

static rstatus_t
process_decr(struct request *req, struct buf *buf)
{
    struct bstring *key;
    struct item *it;
    uint64_t val;

    log_verb("processing decr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);

    if ((it = item_get(key))) {
        if (it->vtype == V_INT) {
            val = _retrieve_item_val_u64(it);

            val -= req->delta;

            _store_item_val_u64(it, val);
            INCR(process_metrics, cmd_decr_stored);
            return compose_rsp_uint64(buf, val, req->noreply);
        }
        /* non integer value */
        log_verb("decr on key %.*s with non integer value", key->len, key->data);
        INCR(process_metrics, cmd_decr_ex);
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    }

    /* item not found */
    INCR(process_metrics, cmd_decr_notfound);
    return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
}

static rstatus_t
process_append(struct request *req, struct buf *buf)
{
    struct bstring *key;
    item_rstatus_t i_status;

    log_verb("processing append req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    i_status = item_annex(key, &(req->vstr), true);

    switch (i_status) {
    case ITEM_OK:
        INCR(process_metrics, cmd_append_stored);
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_ENOTFOUND:
        INCR(process_metrics, cmd_append_notfound);
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    case ITEM_EOVERSIZED:
        INCR(process_metrics, cmd_append_ex);
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        INCR(process_metrics, cmd_append_ex);
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
        break;
    }

    return CC_ERROR;
}

static rstatus_t
process_prepend(struct request *req, struct buf *buf)
{
    struct bstring *key;
    item_rstatus_t i_status;

    log_verb("processing prepend req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    i_status = item_annex(key, &(req->vstr), false);

    switch (i_status) {
    case ITEM_OK:
        INCR(process_metrics, cmd_prepend_stored);
        return compose_rsp_msg(buf, RSP_STORED, req->noreply);
    case ITEM_ENOTFOUND:
        INCR(process_metrics, cmd_prepend_notfound);
        return compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    case ITEM_EOVERSIZED:
        INCR(process_metrics, cmd_prepend_ex);
        return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    case ITEM_ENOMEM:
        INCR(process_metrics, cmd_prepend_ex);
        return compose_rsp_msg(buf, RSP_SERVER_ERROR, req->noreply);
    default:
        NOT_REACHED();
        break;
    }

    return CC_ERROR;
}

static rstatus_t
process_stats(struct request *req, struct buf *buf)
{
    procinfo_update();
    return compose_rsp_stats(buf, (struct metric *)&glob_stats,
                             METRIC_CARDINALITY(glob_stats));
}

static rstatus_t
process_flush(struct request *req, struct buf *buf)
{
    item_flush();
    return compose_rsp_msg(buf, RSP_OK, req->noreply);
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

    case REQ_FLUSH:
        return process_flush(req, buf);

    case REQ_QUIT:
        return CC_ERDHUP;

    default:
        NOT_REACHED();
        break;
    }

    return CC_OK;
}
