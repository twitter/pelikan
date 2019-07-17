#include "process.h"

#include "hotkey/hotkey.h"
#include "protocol/data/memcache_include.h"
#include "storage/cuckoo/cuckoo.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>
#include <time/cc_timer.h>

#define SLIMCACHE_PROCESS_MODULE_NAME "slimcache::process"

#define STORE_ERR_MSG "invalid/oversized value, cannot be stored"
#define DELTA_ERR_MSG "value is not a number"
#define OTHER_ERR_MSG "command not supported"

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;
static bool allow_flush = ALLOW_FLUSH;
static bool prefill = PREFILL;
static uint8_t prefill_ksize;
static char prefill_kbuf[UINT8_MAX]; /* cuckoo storage has klen as uint8_t */
static uint8_t prefill_vsize;
static char prefill_vbuf[UINT8_MAX]; /* cuckoo storage has vlen as uint8_t */
static uint64_t prefill_nkey;


static void
_prefill_cuckoo(void)
{
    struct duration d;
    struct bstring key, vstr;
    struct val val;
    struct item *it;

    duration_reset(&d);
    key.len = prefill_ksize;
    key.data = prefill_kbuf;
    vstr.len = prefill_vsize;
    vstr.data = prefill_vbuf;
    val.type = VAL_TYPE_STR;
    val.vstr = vstr;

    duration_start(&d);
    for (uint32_t i = 0; i < prefill_nkey; ++i) {
        /* print fixed-length key with leading 0's for padding */
        cc_snprintf(&prefill_kbuf, key.len + 1, "%.*d", key.len, i);
        /* fill val, use the same value as key for now */
        cc_snprintf(&prefill_vbuf, vstr.len + 1, "%.*d", vstr.len, i);
        /* insert into cuckoo/heap */
        it = cuckoo_insert(&key, &val,
                time_convert_proc_sec((time_i)INT32_MAX));
        ASSERT(it != NULL);
    }
    duration_stop(&d);

    log_info("prefilling cuckoo with %"PRIu64" keys, of key len %"PRIu8" & val "
            "len %"PRIu8", in %.3f seconds", prefill_nkey, prefill_ksize,
            prefill_vsize, duration_sec(&d));
}


void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", SLIMCACHE_PROCESS_MODULE_NAME);
    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                SLIMCACHE_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
        prefill = option_bool(&options->prefill);
        prefill_ksize = (uint8_t)option_uint(&options->prefill_ksize);
        prefill_vsize = (uint8_t)option_uint(&options->prefill_vsize);
        prefill_nkey = (uint64_t)option_uint(&options->prefill_nkey);
    }

    if (prefill) {
        _prefill_cuckoo();
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
    allow_flush = false;
}


static bool
_get_key(struct response *rsp, struct bstring *key)
{
    struct item *it;
    struct val val;

    it = cuckoo_get(key);
    if (it != NULL) {
        rsp->type = RSP_VALUE;
        rsp->key = *key;
        rsp->flag = item_flag(it);
        rsp->vcas = item_cas(it);
        item_val(&val, it);
        if (val.type == VAL_TYPE_INT) {
            rsp->num = 1;
            rsp->vint = val.vint;
        } else {
            rsp->vstr = val.vstr;
        }

        if (hotkey_enabled && hotkey_sample(key)) {
            log_debug("hotkey detected: %.*s", key->len, key->data);
        }

        log_verb("found key at %p, location %p", key, it);
        return true;
    } else {
        log_verb("key at %p not found", key);
        return false;
    }
}

static void
_process_get(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct response *r = rsp;
    uint32_t i;

    INCR(process_metrics, get);
    /* use chained responses, move to the next response if key is found. */
    for (i = 0; i < array_nelem(req->keys); ++i) {
        INCR(process_metrics, get_key);
        key = array_get(req->keys, i);
        if (_get_key(r, key)) {
            r->cas = false;
            r = STAILQ_NEXT(r, next);
            if (r == NULL) {
                INCR(process_metrics, get_ex);
                log_warn("get response incomplete due to lack of rsp objects");
                return;
            }
            req->nfound++;
            INCR(process_metrics, get_key_hit);
        } else {
            INCR(process_metrics, get_key_miss);
        }
    }
    r->type = RSP_END;

    log_verb("get req %p processed, %d out of %d keys found", req, req->nfound, i);
}

static void
_process_gets(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct response *r = rsp;
    uint32_t i;

    INCR(process_metrics, gets);
    /* use chained responses, move to the next response if key is found. */
    for (i = 0; i < array_nelem(req->keys); ++i) {
        INCR(process_metrics, gets_key);
        key = array_get(req->keys, i);
        if (_get_key(r, key)) {
            r->cas = true;
            r = STAILQ_NEXT(r, next);
            if (r == NULL) {
                INCR(process_metrics, gets_ex);
                log_warn("gets response incomplete due to lack of rsp objects");
            }
            req->nfound++;
            INCR(process_metrics, gets_key_hit);
        } else {
            INCR(process_metrics, gets_key_miss);
        }
    }
    r->type = RSP_END;

    log_verb("gets req %p processed, %d out of %d keys found", req, req->nfound, i);
}

static void
_process_delete(struct response *rsp, struct request *req)
{
    INCR(process_metrics, delete);
    if (cuckoo_delete(array_first(req->keys))) {
        rsp->type = RSP_DELETED;
        INCR(process_metrics, delete_deleted);
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, delete_notfound);
    }

    log_verb("delete req %p processed, rsp type %d", req, rsp->type);
}

static void
_get_value(struct val *val, struct bstring *vstr)
{
    rstatus_i status;

    log_verb("processing value at %p, store at %p", vstr, val);

    status = bstring_atou64(&val->vint, vstr);
    if (status == CC_OK) {
        val->type = VAL_TYPE_INT;
    } else {
        val->type = VAL_TYPE_STR;
        val->vstr = *vstr;
    }
}

static inline void
_error_rsp(struct response *rsp, char *msg)
{
    INCR(process_metrics, process_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(msg);
}

static void
_process_set(struct response *rsp, struct request *req)
{
    rstatus_i status = CC_OK;
    proc_time_i expire;
    struct bstring *key;
    struct item *it;
    struct val val;

    INCR(process_metrics, set);
    key = array_first(req->keys);
    expire = time_convert_proc_sec((time_i)req->expiry);
    _get_value(&val, &req->vstr);

    it = cuckoo_get(key);
    if (it != NULL) {
        status = cuckoo_update(it, &val, expire);
    } else {
        it = cuckoo_insert(key, &val, expire);
    }

    if (it != NULL && status == CC_OK) {
        rsp->type = RSP_STORED;
        INCR(process_metrics, set_stored);
    } else {
        _error_rsp(rsp, STORE_ERR_MSG);
        INCR(process_metrics, set_ex);
    }

    log_verb("set req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_add(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct item *it;
    struct val val;

    INCR(process_metrics, add);
    key = array_first(req->keys);
    it = cuckoo_get(key);
    if (it != NULL) {
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, add_notstored);
    } else {
        _get_value(&val, &req->vstr);
        if (cuckoo_insert(key, &val, time_convert_proc_sec((time_i)req->expiry))
                != NULL) {
            rsp->type = RSP_STORED;
            INCR(process_metrics, add_stored);
        } else {
            _error_rsp(rsp, STORE_ERR_MSG);
            INCR(process_metrics, add_ex);
        }
    }

    log_verb("add req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_replace(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct item *it;
    struct val val;

    INCR(process_metrics, replace);
    key = array_first(req->keys);
    it = cuckoo_get(key);
    if (it != NULL) {
        _get_value(&val, &req->vstr);
        if (cuckoo_update(it, &val, time_convert_proc_sec((time_i)req->expiry))
                == CC_OK) {
            rsp->type = RSP_STORED;
            INCR(process_metrics, replace_stored);
        } else {
            _error_rsp(rsp, STORE_ERR_MSG);
            INCR(process_metrics, replace_ex);
        }
    } else {
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, replace_notstored);
    }

    log_verb("replace req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_cas(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct item *it;
    struct val val;

    INCR(process_metrics, cas);
    key = array_first(req->keys);
    it = cuckoo_get(key);
    if (it != NULL) {

        if (item_cas_valid(it, req->vcas)) {
            _get_value(&val, &req->vstr);
            if (cuckoo_update(it, &val, time_convert_proc_sec((time_i)req->expiry))
                    == CC_OK) {
                rsp->type = RSP_STORED;
                INCR(process_metrics, cas_stored);
            } else {
                _error_rsp(rsp, STORE_ERR_MSG);
                INCR(process_metrics, cas_ex);
            }
        } else {
            rsp->type = RSP_EXISTS;
            INCR(process_metrics, cas_exists);
        }
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, cas_notfound);
    }

    log_verb("cas req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_incr(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct item *it;
    struct val nval;

    INCR(process_metrics, incr);
    key = array_first(req->keys);
    it = cuckoo_get(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            _error_rsp(rsp, DELTA_ERR_MSG);
            INCR(process_metrics, incr_ex);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply incr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return;
        }

        nval.type = VAL_TYPE_INT;
        nval.vint = item_value_int(it) + req->delta;
        item_value_update(it, &nval);
        rsp->type = RSP_NUMERIC;
        rsp->vint = nval.vint;
        INCR(process_metrics, incr_stored);
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, incr_notfound);
    }

    log_verb("incr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_decr(struct response *rsp, struct request *req)
{
    struct bstring *key;
    struct item *it;
    uint64_t v;
    struct val nval;

    INCR(process_metrics, decr);
    key = array_first(req->keys);
    it = cuckoo_get(key);
    if (NULL != it) {
        if (item_vtype(it) != VAL_TYPE_INT) {
            _error_rsp(rsp, DELTA_ERR_MSG);
            INCR(process_metrics, decr_ex);
            /* TODO(yao): binary key */
            log_warn("value not int, cannot apply decr on key %.*s val %.*s",
                    key->len, key->data, it->vlen, ITEM_VAL_POS(it));
            return;
        }

        v = item_value_int(it);
        nval.type = VAL_TYPE_INT;
        if (v < req->delta) {
            nval.vint = 0;
        } else {
            nval.vint = v - req->delta;
        }
        item_value_update(it, &nval);
        rsp->type = RSP_NUMERIC;
        rsp->vint = nval.vint;
        INCR(process_metrics, decr_stored);
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, decr_notfound);
    }

    log_verb("incr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_flush(struct response *rsp, struct request *req)
{
    if (allow_flush) {
        INCR(process_metrics, flush);
        cuckoo_reset();
        rsp->type = RSP_OK;

        log_info("flush req %p processed, rsp type %d", req, rsp->type);
    } else {
        _error_rsp(rsp, OTHER_ERR_MSG);
    }
}

void
process_request(struct response *rsp, struct request *req)
{
    log_verb("processing req %p, write rsp to %p", req, rsp);
    INCR(process_metrics, process_req);

    switch (req->type) {
    case REQ_GET:
        _process_get(rsp, req);
        break;

    case REQ_GETS:
        _process_gets(rsp, req);
        break;

    case REQ_DELETE:
        _process_delete(rsp, req);
        break;

    case REQ_SET:
        _process_set(rsp, req);
        break;

    case REQ_ADD:
        _process_add(rsp, req);
        break;

    case REQ_REPLACE:
        _process_replace(rsp, req);
        break;

    case REQ_CAS:
        _process_cas(rsp, req);
        break;

    case REQ_INCR:
        _process_incr(rsp, req);
        break;

    case REQ_DECR:
        _process_decr(rsp, req);
        break;

    case REQ_FLUSH:
        _process_flush(rsp, req);
        break;

    default:
        rsp->type = RSP_CLIENT_ERROR;
        rsp->vstr = str2bstr(OTHER_ERR_MSG);
        break;
    }
}

static inline void
_cleanup(struct request **req, struct response **rsp)
{
    request_return(req);
    response_return_all(rsp);
}

int
slimcache_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_e status;
    struct request *req;
    struct response *rsp = NULL;

    log_verb("post-read processing");

    req = request_borrow();
    if (req == NULL) {
        /* TODO(yao): simply return for now, better to respond with OOM */
        log_error("cannot acquire request: OOM");
        INCR(process_metrics, process_ex);

        return -1;
    }

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(*rbuf) > 0) {
        char *old_rpos;
        struct response *nr;
        int i, card;

        /* stage 1: parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(*rbuf));

        old_rpos = (*rbuf)->rpos;
        status = parse_req(req, *rbuf);
        if (status == PARSE_EUNFIN || req->partial) { /* ignore partial */
            (*rbuf)->rpos = old_rpos;
            buf_lshift(*rbuf);
            return 0;
        }
        if (status != PARSE_OK) {
            /* parsing errors are all client errors, since we don't know
             * how to recover from client errors in this condition (we do not
             * have a valid request so we don't know where the invalid request
             * ends), we should close the connection
             */
            log_warn("illegal request received, status: %d", status);
            goto error;
        }

        if (req->swallow) { /* skip to the end of current request */
            continue;
        }

        /* stage 2: processing- check for quit, allocate response(s), process */

        /* quit is special, no response expected */
        if (req->type == REQ_QUIT) {
            log_info("peer called quit");
            goto error;
        }

        /* find cardinality of the request and get enough response objects */
        card = array_nelem(req->keys);
        if (req->type == REQ_GET || req->type == REQ_GETS) {
            /* extra response object for the "END" line after values */
            card++;
        }
        for (i = 0, rsp = response_borrow(), nr = rsp;
             i < card;
             i++, STAILQ_NEXT(nr, next) = response_borrow(), nr =
                STAILQ_NEXT(nr, next)) {
            if (nr == NULL) {
                log_error("cannot acquire response: OOM");
                INCR(process_metrics, process_ex);
                goto error;
            }
        }

        /* actual processing & command logging */
        process_request(rsp, req);

        /* stage 3: write response(s) */

        /* noreply means no need to write to buffers */
        if (!req->noreply) {
            nr = rsp;
            if (req->type == REQ_GET || req->type == REQ_GETS) {
                /* for get/gets, card is determined by number of values */
                card = req->nfound + 1;
            }
            for (i = 0; i < card; nr = STAILQ_NEXT(nr, next), ++i) {
                if (compose_rsp(wbuf, nr) < 0) {
                    log_error("composing rsp erred");
                    INCR(process_metrics, process_ex);
                    goto error;
                }
            }
        }

        /* logging, clean-up */
        klog_write(req, rsp);
        _cleanup(&req, &rsp);
    }

    return 0;

error:
    _cleanup(&req, &rsp);
    return -1;
}

int
slimcache_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}

int
slimcache_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}
