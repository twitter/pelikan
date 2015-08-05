#include <slimcache/process.h>

#include <protocol/memcache/codec.h>
#include <slimcache/stats.h>
#include <storage/cuckoo/cuckoo.h>
#include <util/procinfo.h>

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>

#define SLIMCACHE_PROCESS_MODULE_NAME "slimcache::process"

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;

void
process_setup(process_metrics_st *metrics)
{
    log_info("set up the %s module", SLIMCACHE_PROCESS_MODULE_NAME);

    process_metrics = metrics;
    PROCESS_METRIC_INIT(process_metrics);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                SLIMCACHE_PROCESS_MODULE_NAME);
    }
    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", SLIMCACHE_PROCESS_MODULE_NAME);

    if (!process_init) {
        log_warn("%s has never been setup", SLIMCACHE_PROCESS_MODULE_NAME);
    }
    process_metrics = NULL;
    process_init = false;
}

static int
process_get_key(struct buf *buf, struct bstring *key)
{
    int ret = 0;
    struct item *it;
    struct val val;
    uint8_t val_str[CC_UINT64_MAXLEN];
    size_t size;

    log_verb("get key at %p, rsp buf at %p", key, buf);
    INCR(process_metrics, cmd_get_key);

    it = cuckoo_lookup(key);
    if (NULL != it) {
        log_verb("found key at item %p", it);
        INCR(process_metrics, cmd_get_key_hit);

        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) { /* print and overwrite val */
            size = cc_scnprintf(val_str, CC_UINT64_MAXLEN, "%"PRIu64, val.vint);
            val.vstr.data = val_str;
            val.vstr.len = (uint32_t)size;
        }

        ret = compose_rsp_keyval(buf, key, &val.vstr, item_flag(it), 0);
    } else {
        INCR(process_metrics, cmd_get_key_miss);
    }

    return ret;
}

static int
process_get(struct request *req, struct buf *buf)
{
    int status, ret = 0;
    struct bstring *key;
    uint32_t i;

    log_verb("processing get req %p, rsp buf at %p", req, buf);

    for (i = 0; i < req->keys->nelem; ++i) {
        key = array_get_idx(req->keys, i);
        status = process_get_key(buf, key);
        if (status < 0) {
            return status;
        }
        ret += status;
    }

    status = compose_rsp_msg(buf, RSP_END, false);
    if (status < 0) {
        return status;
    }

    return ret + status;
}

static int
process_gets_key(struct buf *buf, struct bstring *key)
{
    int ret = 0;
    struct item *it;
    struct val val;
    uint8_t val_str[CC_UINT64_MAXLEN];
    size_t size;

    log_verb("gets key at %p, rsp buf at %p", key, buf);
    INCR(process_metrics, cmd_gets_key);

    it = cuckoo_lookup(key);
    if (NULL != it) {
        INCR(process_metrics, cmd_gets_key_hit);

        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) { /* print and overwrite val */
            size = cc_scnprintf(val_str, CC_UINT64_MAXLEN, "%"PRIu64, val.vint);
            val.vstr.data = val_str;
            val.vstr.len = (uint32_t)size;
        }

        ret = compose_rsp_keyval(buf, key, &val.vstr, item_flag(it),
                                 item_cas(it));
    } else {
        INCR(process_metrics, cmd_gets_key_miss);
    }

    return ret;
}

static int
process_gets(struct request *req, struct buf *buf)
{
    int status, ret = 0;
    struct bstring *key;
    uint32_t i;

    log_verb("processing gets req %p, rsp buf at %p", req, buf);

    for (i = 0; i < req->keys->nelem; ++i) {
        key = array_get_idx(req->keys, i);
        status = process_gets_key(buf, key);
        if (status < 0) {
            return status;
        }
        ret += status;
    }

    status = compose_rsp_msg(buf, RSP_END, false);
    if (status < 0) {
        return status;
    }

    return ret + status;
}

static int
process_delete(struct request *req, struct buf *buf)
{
    int ret;
    bool deleted;

    log_verb("processing delete req %p, rsp buf at %p", req, buf);

    deleted = cuckoo_delete(array_get_idx(req->keys, 0));
    if (deleted) {
        INCR(process_metrics, cmd_delete_deleted);
        ret = compose_rsp_msg(buf, RSP_DELETED, req->noreply);
    } else {
        INCR(process_metrics, cmd_delete_notfound);
        ret = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return ret;
}

static void
process_value(struct val *val, struct bstring *val_str)
{
    rstatus_t status;

    log_verb("processing value at %p, store at %p", val_str, val);

    status = bstring_atou64(&val->vint, val_str);
    if (status == CC_OK) {
        val->type = VAL_TYPE_INT;
    } else {
        val->type = VAL_TYPE_STR;
        val->vstr = *val_str;
    }
}

static int
process_set(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    int ret;
    rel_time_t expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    log_verb("processing set req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    expire = time_reltime(req->expiry);
    process_value(&val, &req->vstr);

    it = cuckoo_lookup(key);
    if (it != NULL) {
        status = cuckoo_update(it, &val, expire);
    } else {
        status = cuckoo_insert(key, &val, expire);
    }

    if (status == CC_OK) {
        INCR(process_metrics, cmd_set_stored);
        ret = compose_rsp_msg(buf, RSP_STORED, req->noreply);
    } else {
        INCR(process_metrics, cmd_set_ex);
        ret = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    }

    return ret;
}

static int
process_add(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    int ret;
    rel_time_t expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    log_verb("processing add req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (it != NULL) {
        INCR(process_metrics, cmd_add_notstored);
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    } else {
        expire = time_reltime(req->expiry);
        process_value(&val, &req->vstr);
        status = cuckoo_insert(key, &val, expire);
        if (status == CC_OK) {
            INCR(process_metrics, cmd_add_stored);
            ret = compose_rsp_msg(buf, RSP_STORED, req->noreply);
        } else {
            INCR(process_metrics, cmd_add_ex);
            ret = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }
    }

    return ret;
}

static int
process_replace(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    int ret;
    rel_time_t expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    log_verb("processing replace req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (it != NULL) {
        expire = time_reltime(req->expiry);
        process_value(&val, &req->vstr);
        status = cuckoo_update(it, &val, expire);
        if (status == CC_OK) {
            INCR(process_metrics, cmd_replace_stored);
            ret = compose_rsp_msg(buf, RSP_STORED, req->noreply);
        } else {
            INCR(process_metrics, cmd_replace_ex);
            ret = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }
    } else {
        INCR(process_metrics, cmd_replace_notstored);
        ret = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    }

    return ret;
}

static int
process_cas(struct request *req, struct buf *buf)
{
    rstatus_t status = CC_OK;
    int ret;
    rel_time_t expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    log_verb("processing cas req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (it != NULL) {
        if (item_cas_valid(it, req->cas)) {
            expire = time_reltime(req->expiry);
            process_value(&val, &req->vstr);
            status = cuckoo_update(it, &val, expire);
            if (status == CC_OK) {
                INCR(process_metrics, cmd_cas_stored);
                ret = compose_rsp_msg(buf, RSP_STORED, req->noreply);
            } else {
                INCR(process_metrics, cmd_cas_ex);
                ret = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
            }
        } else {
            INCR(process_metrics, cmd_cas_exists);
            ret = compose_rsp_msg(buf, RSP_EXISTS, req->noreply);
        }
    } else {
        INCR(process_metrics, cmd_cas_notfound);
        ret = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return ret;
}

static int
process_incr(struct request *req, struct buf *buf)
{
    int status;
    struct bstring *key;
    struct item *it;
    struct val new_val;

    log_verb("processing incr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            INCR(process_metrics, cmd_incr_ex);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply incr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }

        new_val.type = VAL_TYPE_INT;
        new_val.vint = item_value_int(it) + req->delta;
        item_value_update(it, &new_val);
        INCR(process_metrics, cmd_incr_stored);
        status = compose_rsp_uint64(buf, new_val.vint, req->noreply);
    } else {
        INCR(process_metrics, cmd_incr_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
}

static int
process_decr(struct request *req, struct buf *buf)
{
    int status;
    struct bstring *key;
    struct item *it;
    struct val new_val;

    log_verb("processing decr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            INCR(process_metrics, cmd_decr_ex);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply decr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }

        new_val.type = VAL_TYPE_INT;
        new_val.vint = item_value_int(it) - req->delta;
        item_value_update(it, &new_val);
        INCR(process_metrics, cmd_decr_stored);
        status = compose_rsp_uint64(buf, new_val.vint, req->noreply);
    } else {
        INCR(process_metrics, cmd_decr_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
}

static int
process_stats(struct request *req, struct buf *buf)
{
    procinfo_update();
    return compose_rsp_stats(buf, (struct metric *)&glob_stats,
            METRIC_CARDINALITY(glob_stats));
}

static int
process_flush(struct request *req, struct buf *buf)
{
    cuckoo_reset();
    return compose_rsp_msg(buf, RSP_OK, req->noreply);
}

int
process_request(struct request *req, struct buf *buf)
{
    log_verb("processing req %p, rsp buf at %p", req, buf);
    INCR(process_metrics, cmd_process);

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
