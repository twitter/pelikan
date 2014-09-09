#include <slimcache/bb_process.h>

#include <cuckoo/bb_cuckoo.h>

#include <cc_array.h>
#include <cc_log.h>
#include <cc_print.h>

static rstatus_t
process_get_key(struct mbuf *buf, struct bstring *key, bool cas)
{
    rstatus_t status = CC_OK;
    struct item *it;
    struct val val;
    uint8_t val_str[CC_UINT64_MAXLEN];
    size_t size;

    it = cuckoo_lookup(key);
    if (NULL != it) {
        //if (cas) {
        //    stats_thread_incr(gets_key_hit);
        //} else {
        //    stats_thread_incr_get_key_hit);
        //}

        status = rsp_write_msg(buf, RSP_VALUE);
        if (status != CC_OK) {
            return status;
        }

        status = rsp_write_bstring(buf, key);
        if (status != CC_OK) {
            return status;
        }

        status = rsp_write_uint64(buf, item_flag(it)); /* NOTE: value is a uint32_t */
        if (status != CC_OK) {
            return status;
        }

        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) { /* print and overwrite val */
            size = cc_scnprintf(val_str, CC_UINT64_MAXLEN, "%"PRIu64, val.vint);
            val.vstr.data = val_str;
            val.vstr.len = (uint32_t)size;
        }

        status = rsp_write_uint64(buf, val.vstr.len);
        if (status != CC_OK) {
            return status;
        }

        status = rsp_write_msg(buf, RSP_CRLF);
        if (status != CC_OK) {
            return status;
        }

        status = rsp_write_bstring(buf, &val.vstr);
        if (status != CC_OK) {
            return status;
        }

        status = rsp_write_bstring(buf, RSP_CRLF);
        if (status != CC_OK) {
            return status;
        }
    } else {
        //if (cas) {
        //    stats_thread_incr(gets_key_miss);
        //} else {
        //    stats_thread_incr_get_key_miss);
        //}
    }

    return status;
}

static rstatus_t
process_get(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    struct bstring *key;
    uint32_t i;

    //stats_thread_incr(get);

    for (i = 0; i < req->keys->nelem; ++i) {

        key = array_get_idx(req->keys, i);
        status = process_get_key(buf, key, false);

    }
    status = rsp_write_msg(buf, RSP_END);

    return status;
}

static rstatus_t
process_gets(struct request *req, struct mbuf *buf)
{
    rstatus_t status;
    struct bstring *key;
    uint32_t i;

    //stats_thread_incr(gets);

    for (i = 0; i < req->keys->nelem; ++i) {
        //stats_thread_incr(gets_key);

        key = array_get_idx(req->keys, i);
        status = process_get_key(buf, key, true);

    }
    status = rsp_write_msg(buf, RSP_END);

    return status;
}

static rstatus_t
process_delete(struct request *req, struct mbuf *buf)
{
    rstatus_t status = CC_OK;
    struct item *it;

    it = cuckoo_lookup(array_get_idx(req->keys, 0));
    if (NULL != it) {
        //stats_thread_incr(delete_hit);

        item_delete(it);

        if (!req->noreply) {
            status = rsp_write_msg(buf, RSP_DELETED);
        }
    } else {
        //stats_thread_incr(delete_miss);

        if (!req->noreply) {
            status = rsp_write_msg(buf, RSP_NOT_FOUND);
        }
    }

    return status;
}

rstatus_t
process_request(struct request *req, struct mbuf *buf)
{
    rstatus_t status;

    switch (req->verb) {
    case GET:
        status = process_get(req, buf);

        return status;

    case GETS:
        status = process_gets(req, buf);

        return status;

    case DELETE:
        status = process_delete(req, buf);

        return status;

    default:
        NOT_REACHED();
        break;
    }

    return CC_OK;
}
