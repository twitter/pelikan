#include <slimcache/bb_process.h>

#include <bb_stats.h>
#include <cuckoo/bb_cuckoo.h>
#include <slimcache/bb_stats.h>

#include <cc_array.h>
#include <cc_log.h>
#include <cc_print.h>

#include <sys/resource.h>
#include <unistd.h>

#define USEC 0.000001

static rstatus_t
process_get_key(struct mbuf *buf, struct bstring *key)
{
    rstatus_t status = CC_OK;
    struct item *it;
    struct val val;
    uint8_t val_str[CC_UINT64_MAXLEN];
    size_t size;

    log_verb("get key at %p, rsp buf at %p", key, buf);
    INCR(get_key);

    it = cuckoo_lookup(key);
    if (NULL != it) {
        log_verb("found key at item %p");
        INCR(get_key_hit);

        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) { /* print and overwrite val */
            size = cc_scnprintf(val_str, CC_UINT64_MAXLEN, "%"PRIu64, val.vint);
            val.vstr.data = val_str;
            val.vstr.len = (uint32_t)size;
        }

        status = compose_rsp_keyval(buf, key, &val.vstr, item_flag(it), 0);
    } else {
        INCR(get_key_miss);
    }

    return status;
}

static rstatus_t
process_get(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    struct bstring *key;
    uint32_t i;

    log_verb("processing get req %p, rsp buf at %p", req, buf);

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
process_gets_key(struct mbuf *buf, struct bstring *key)
{
    rstatus_t status = CC_OK;
    struct item *it;
    struct val val;
    uint8_t val_str[CC_UINT64_MAXLEN];
    size_t size;

    log_verb("gets key at %p, rsp buf at %p", key, buf);
    INCR(get_key_hit);

    it = cuckoo_lookup(key);
    if (NULL != it) {
        INCR(gets_key_hit);

        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) { /* print and overwrite val */
            size = cc_scnprintf(val_str, CC_UINT64_MAXLEN, "%"PRIu64, val.vint);
            val.vstr.data = val_str;
            val.vstr.len = (uint32_t)size;
        }

        status = compose_rsp_keyval(buf, key, &val.vstr, item_flag(it),
                item_cas(it));
    } else {
        INCR(gets_key_miss);
    }

    return status;
}

static rstatus_t
process_gets(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
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

    return status;
}

static rstatus_t
process_delete(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
    bool deleted;

    log_verb("processing delete req %p, rsp buf at %p", req, buf);

    deleted = cuckoo_delete(array_get_idx(req->keys, 0));
    if (deleted) {
        INCR(delete_deleted);
        status = compose_rsp_msg(buf, RSP_DELETED, req->noreply);
    } else {
        INCR(delete_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
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

static rstatus_t
process_set(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
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
        INCR(set_stored);
        status = compose_rsp_msg(buf, RSP_STORED, req->noreply);
    } else {
        INCR(set_error);
        status = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
    }

    return status;
}

static rstatus_t
process_add(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
    rel_time_t expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    log_verb("processing add req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (it != NULL) {
        INCR(add_notstored);
        status = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    } else {
        expire = time_reltime(req->expiry);
        process_value(&val, &req->vstr);
        status = cuckoo_insert(key, &val, expire);
        if (status == CC_OK) {
            INCR(add_stored);
            status = compose_rsp_msg(buf, RSP_STORED, req->noreply);
        } else {
            INCR(add_error);
            status = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }
    }

    return status;
}

static rstatus_t
process_replace(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
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
            INCR(replace_stored);
            status = compose_rsp_msg(buf, RSP_STORED, req->noreply);
        } else {
            INCR(replace_error);
            status = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }
    } else {
        INCR(replace_notstored);
        status = compose_rsp_msg(buf, RSP_NOT_STORED, req->noreply);
    }

    return status;
}

static rstatus_t
process_cas(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
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
                INCR(cas_stored);
                status = compose_rsp_msg(buf, RSP_STORED, req->noreply);
            } else {
                INCR(cas_error);
                status = compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
            }
        } else {
            INCR(cas_exists);
            status = compose_rsp_msg(buf, RSP_EXISTS, req->noreply);
        }
    } else {
        INCR(cas_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
}

static rstatus_t
process_incr(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
    struct bstring *key;
    struct item *it;
    struct val new_val;

    log_verb("processing incr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            INCR(incr_error);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply incr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }

        new_val.type = VAL_TYPE_INT;
        new_val.vint = item_value_int(it) + req->delta;
        item_value_update(it, &new_val);
        INCR(incr_stored);
        status = compose_rsp_uint64(buf, new_val.vint, req->noreply);
    } else {
        INCR(incr_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
}

static rstatus_t
process_decr(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
    struct bstring *key;
    struct item *it;
    struct val new_val;

    log_verb("processing decr req %p, rsp buf at %p", req, buf);

    key = array_get_idx(req->keys, 0);
    it = cuckoo_lookup(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            INCR(decr_error);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply decr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return compose_rsp_msg(buf, RSP_CLIENT_ERROR, req->noreply);
        }

        new_val.type = VAL_TYPE_INT;
        new_val.vint = item_value_int(it) - req->delta;
        item_value_update(it, &new_val);
        INCR(decr_stored);
        status = compose_rsp_uint64(buf, new_val.vint, req->noreply);
    } else {
        INCR(decr_notfound);
        status = compose_rsp_msg(buf, RSP_NOT_FOUND, req->noreply);
    }

    return status;
}

static rstatus_t
process_stats(struct request *req, struct mbuf *buf)
{
    struct rusage usage;

    Stats.pid.vintmax         = (intmax_t)getpid();
    Stats.time.vintmax        = (intmax_t)time_started() + time_now();
    Stats.uptime.vintmax      = (intmax_t)time_now();
    /* "%02d%02d%02d", MC_VERSION_MAJOR, MC_VERSION_MINOR, MC_VERSION_PATCH */
    Stats.version.vintmax     = (intmax_t)BB_VERSION_MAJOR * 10000 +
                                    BB_VERSION_MINOR * 100 + BB_VERSION_PATCH;

    /* TODO(yao): put system stats in a single section, potentially a library */
    getrusage(RUSAGE_SELF, &usage);
    Stats.ru_utime.vdouble    = (double)usage.ru_utime.tv_sec +
                                    (double)usage.ru_utime.tv_usec * USEC;
    Stats.ru_stime.vdouble    = (double)usage.ru_stime.tv_sec +
                                    (double)usage.ru_stime.tv_usec * USEC;
    Stats.ru_maxrss.vintmax   = (intmax_t)usage.ru_maxrss;
    Stats.ru_ixrss.vintmax    = (intmax_t)usage.ru_ixrss;
    Stats.ru_idrss.vintmax    = (intmax_t)usage.ru_idrss;
    Stats.ru_isrss.vintmax    = (intmax_t)usage.ru_isrss;
    Stats.ru_minflt.vintmax   = (intmax_t)usage.ru_minflt;
    Stats.ru_majflt.vintmax   = (intmax_t)usage.ru_majflt;
    Stats.ru_nswap.vintmax    = (intmax_t)usage.ru_nswap;
    Stats.ru_inblock.vintmax  = (intmax_t)usage.ru_inblock;
    Stats.ru_oublock.vintmax  = (intmax_t)usage.ru_oublock;
    Stats.ru_msgsnd.vintmax   = (intmax_t)usage.ru_msgsnd;
    Stats.ru_msgrcv.vintmax   = (intmax_t)usage.ru_msgrcv;
    Stats.ru_nsignals.vintmax = (intmax_t)usage.ru_nsignals;
    Stats.ru_nvcsw.vintmax    = (intmax_t)usage.ru_nvcsw;
    Stats.ru_nivcsw.vintmax   = (intmax_t)usage.ru_nivcsw;

    return compose_rsp_stats(buf, (struct metric *)&Stats, Nmetric);
}

rstatus_t
process_request(struct request *req, struct mbuf *buf)
{
    rstatus_t status;

    log_verb("processing req %p, rsp buf at %p", req, buf);

    switch (req->verb) {
    case REQ_GET:
        status = process_get(req, buf);

        return status;

    case REQ_GETS:
        status = process_gets(req, buf);

        return status;

    case REQ_DELETE:
        status = process_delete(req, buf);

        return status;

    case REQ_SET:
        status = process_set(req, buf);

        return status;

    case REQ_ADD:
        status = process_add(req, buf);

        return status;

    case REQ_REPLACE:
        status = process_replace(req, buf);

        return status;

    case REQ_CAS:
        status = process_cas(req, buf);

        return status;

    case REQ_INCR:
        status = process_incr(req, buf);

        return status;

    case REQ_DECR:
        status = process_decr(req, buf);

        return status;

    case REQ_STATS:
        status = process_stats(req, buf);

        return status;

    case REQ_QUIT:
        return CC_ERDHUP;

    default:
        NOT_REACHED();
        break;
    }

    return CC_OK;
}
