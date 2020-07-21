#include "process.h"

#include "hotkey/hotkey.h"
#include "protocol/data/memcache_include.h"
#include "storage/seg/seg.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>
#include <time/cc_timer.h>

#define SEGCACHE_PROCESS_MODULE_NAME "segcache::process"

#define OVERSIZE_ERR_MSG "oversized value, cannot be stored"
#define DELTA_ERR_MSG "value is not a number"
#define OOM_ERR_MSG "server is out of memory"
#define CMD_ERR_MSG "command not supported"
#define OTHER_ERR_MSG "unknown server error"

typedef enum put_rstatus {
    PUT_OK,
    PUT_PARTIAL,
    PUT_ERROR,
} put_rstatus_e;

static bool                 process_init = false;
static process_metrics_st   *process_metrics = NULL;
static bool                 allow_flush = ALLOW_FLUSH;
static bool                 prefill = PREFILL;
static uint32_t             prefill_ksize;
static char                 prefill_kbuf[UINT8_MAX]; /* seg implementation has klen as unint8_t
                                      */
static uint32_t             prefill_vsize;
/* val_buf size is arbitrary , update if want to warm up with larger objects */
static char                 prefill_vbuf[ITEM_SIZE_MAX];
static uint64_t             prefill_nkey;

static void
_prefill_seg(void)
{
    struct duration d;
    struct bstring key, val;
    struct item *it;

    duration_reset(&d);
    key.len = prefill_ksize;
    key.data = prefill_kbuf;
    val.len = prefill_vsize;
    val.data = prefill_vbuf;

    duration_start(&d);
    for (uint32_t i = 0; i < prefill_nkey; ++i) {
        /* print fixed-length key with leading 0's for padding */
        cc_snprintf(&prefill_kbuf, key.len + 1, "%.*d", key.len, i);
        /* fill val, use the same value as key for now */
        cc_snprintf(&prefill_vbuf, val.len + 1, "%.*d", val.len, i);
        /* insert into seg/heap */
        item_reserve(&it, &key, &val, val.len, DATAFLAG_SIZE,
                time_convert_proc_sec((time_i)INT32_MAX));
        ASSERT(it != NULL);
        item_insert(it);
    }
    duration_stop(&d);

    log_info("prefilling seg with %" PRIu64 " keys, of key len %" PRIu32 " & "
             "val len %" PRIu32 ", in %.3f seconds",
            prefill_nkey, prefill_ksize, prefill_vsize, duration_sec(&d));
}

void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", SEGCACHE_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                SEGCACHE_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
        prefill = option_bool(&options->prefill);
        prefill_ksize = (uint32_t)option_uint(&options->prefill_ksize);
        prefill_vsize = (uint32_t)option_uint(&options->prefill_vsize);
        prefill_nkey = (uint64_t)option_uint(&options->prefill_nkey);
    }

    if (prefill) {
        _prefill_seg();
    }

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", SEGCACHE_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", SEGCACHE_PROCESS_MODULE_NAME);
    }

    allow_flush = false;
    process_metrics = NULL;
    process_init = false;
}

static inline uint32_t
_get_dataflag(struct item *it)
{
    uint32_t *p = (uint32_t *)item_optional(it);
    if (p != NULL) {
        return *p;
    } else {
        return 0;
    }
}

static inline void
_set_dataflag(struct item *it, uint32_t flag)
{
    it->olen = sizeof(flag);
    uint32_t *p = (uint32_t *)item_optional(it);
    *p = flag;
}

static bool
_get_key(struct response *rsp, struct bstring *key, bool cas)
{
    struct item *it;
    uint64_t cas_v;

    it = item_get(key, &cas_v, true);
    if (it != NULL) {
        rsp->type = RSP_VALUE;
        rsp->key = *key;
        rsp->flag = _get_dataflag(it);
        rsp->vstr.len = it->vlen; /* do not use item_nval here */
        rsp->vstr.data = item_val(it);
        rsp->vcas = cas ? cas_v : 0;

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
        if (_get_key(r, key, false)) {
            req->nfound++;
            r->cas = false;
            r = STAILQ_NEXT(r, next);
            if (r == NULL) {
                INCR(process_metrics, get_ex);
                log_warn("get response incomplete due to lack of rsp objects");
                return;
            }
            INCR(process_metrics, get_key_hit);
        } else {
            INCR(process_metrics, get_key_miss);
        }
    }
    r->type = RSP_END;

    log_verb("get req %p processed, %d out of %d keys found", req, req->nfound,
            i);
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
        if (_get_key(r, key, true)) {
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

    log_verb("gets req %p processed, %d out of %d keys found", req, req->nfound,
            i);
}

static void
_process_delete(struct response *rsp, struct request *req)
{
    INCR(process_metrics, delete);
    if (item_delete(array_first(req->keys))) {
        /* TODO(jason): why only delete the first */
        rsp->type = RSP_DELETED;
        INCR(process_metrics, delete_deleted);
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, delete_notfound);
    }

    log_verb("delete req %p processed, rsp type %d", req, rsp->type);
}

static void
_error_rsp(struct response *rsp, item_rstatus_e status)
{
    INCR(process_metrics, process_ex);

    if (status == ITEM_EOVERSIZED) {
        rsp->type = RSP_CLIENT_ERROR;
        rsp->vstr = str2bstr(OVERSIZE_ERR_MSG);
    } else if (status == ITEM_ENAN) {
        rsp->type = RSP_CLIENT_ERROR;
        rsp->vstr = str2bstr(DELTA_ERR_MSG);
    } else if (status == ITEM_ENOMEM) {
        rsp->type = RSP_SERVER_ERROR;
        rsp->vstr = str2bstr(OOM_ERR_MSG);
        INCR(process_metrics, process_server_ex);
    } else {
        NOT_REACHED();
        rsp->type = RSP_SERVER_ERROR;
        rsp->vstr = str2bstr(OTHER_ERR_MSG);
        INCR(process_metrics, process_server_ex);
    }
}

/*
 * for the first segment three return values are possible:
 *   - PUT_OK
 *   - PUT_PARTIAL
 *   - PUT_ERROR (error code given in *istatus)
 *
 * for the following segment(s) two return values are possible:
 *   - PUT_OK
 *   - PUT_PARTIAL
 */
static put_rstatus_e
_put(item_rstatus_e *istatus, struct request *req)
{
    put_rstatus_e status;
    struct item *it = NULL;

    *istatus = ITEM_OK;
    if (req->first) { /* self-contained req */
        struct bstring *key = array_first(req->keys);
        /* TODO(jason): might worthwhile add a new function for cal TTL */
        *istatus = item_reserve(&it, key, &req->vstr, req->vlen, DATAFLAG_SIZE,
                time_convert_proc_sec((time_i)req->expiry));
        req->first = false;
        req->reserved = it;
    } else { /* backfill reserved item */
        it = req->reserved;
        item_backfill(it, &req->vstr);
    }

    if (!req->partial) {
        status = (*istatus == ITEM_OK) ? PUT_OK : PUT_ERROR;
    } else { /* should not update hash */
        status = (*istatus == ITEM_OK) ? PUT_PARTIAL : PUT_ERROR;
    }

    if (status == PUT_ERROR) {
        req->swallow = true;
        req->serror = true;
    }
    if (status == PUT_OK) { /* set flag when put is complete */
        _set_dataflag(it, req->flag);
    }

    return status;
}

/*
 * for set/add/replace/cas, we have to recover key from the reserved item,
 * because the keys field in the request are only valid for the first segment
 * of the request buffer. Once we move to later segments, the areas pointed to
 * by these pointers will be overwritten.
 */
static void
_process_set(struct response *rsp, struct request *req)
{
    put_rstatus_e status;
    item_rstatus_e istatus;
    struct item *it;

    status = _put(&istatus, req);
    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        _error_rsp(rsp, istatus);
        INCR(process_metrics, set_ex);

        return;
    }

    /* PUT_OK, meaning we have an item reserved, i.e. req->reserved != NULL */
    INCR(process_metrics, set);
    it = (struct item *)req->reserved;
    item_insert(it);
    rsp->type = RSP_STORED;
    INCR(process_metrics, set_stored);

    log_verb("set req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_add(struct response *rsp, struct request *req)
{
    put_rstatus_e status;
    item_rstatus_e istatus;
    struct item *it;
    struct bstring *key_p;

    /* jason: different from twemcache in that we check before reserving item
     * because reserving item but not use in segcache will cause space waste */

    if (req->first) {
        INCR(process_metrics, add);
        key_p = array_first(req->keys);
        if (item_get(key_p, NULL, false) != NULL) {
            rsp->type = RSP_NOT_STORED;
            req->swallow = 1;
            INCR(process_metrics, add_notstored);
            return;
        }
        status = _put(&istatus, req);
    } else {
        /* segments left */
        it = req->reserved;
        ASSERT(it != NULL);
        struct bstring key = (struct bstring){it->klen, item_key(it)};
        ASSERT(item_get(&key, NULL, false) == NULL);
        status = _put(&istatus, req);
    }

    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        req->swallow = 1;
        _error_rsp(rsp, istatus);
        INCR(process_metrics, add_ex);

        return;
    }

    it = (struct item *)req->reserved;
    /* TODO(jason): BUG!!! another thread might have inserted before us */
    item_insert(it);
    rsp->type = RSP_STORED;
    INCR(process_metrics, add_stored);

    log_verb("add req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_replace(struct response *rsp, struct request *req)
{
    put_rstatus_e status;
    item_rstatus_e istatus;
    struct item *it = NULL;
    struct bstring *key_p;

    if (req->first) {
        INCR(process_metrics, add);
        key_p = array_first(req->keys);
        if (item_get(key_p, NULL, false) == NULL) {
            rsp->type = RSP_NOT_STORED;
            req->swallow = 1;
            INCR(process_metrics, replace_notstored);
            return;
        }
        status = _put(&istatus, req);
    } else {
        /* segments left */
        status = _put(&istatus, req);
    }

    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        req->swallow = 1;
        _error_rsp(rsp, istatus);
        INCR(process_metrics, replace_ex);

        return;
    }

    it = (struct item *)req->reserved;
    item_insert(it);
    rsp->type = RSP_STORED;
    INCR(process_metrics, replace_stored);

    log_verb("replace req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_cas(struct response *rsp, struct request *req)
{
    put_rstatus_e status;
    item_rstatus_e istatus;
    struct item *it = NULL;
    struct bstring *key_p;
    uint64_t cas;

    if (req->first) {
        INCR(process_metrics, add);
        key_p = array_first(req->keys);
        it = item_get(key_p, &cas, false);
        if (it == NULL) {
            rsp->type = RSP_NOT_FOUND;
            req->swallow = 1;
            INCR(process_metrics, cas_notfound);
            return;
        }

        if (cas != req->vcas) {
            req->swallow = 1;
            rsp->type = RSP_EXISTS;
            INCR(process_metrics, cas_exists);
            return;
        }

        status = _put(&istatus, req);

    } else {
        /* segments left */
        status = _put(&istatus, req);
    }

    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        req->swallow = 1;
        _error_rsp(rsp, istatus);
        INCR(process_metrics, cas_ex);

        return;
    }

    it = (struct item *)req->reserved;
    /* TODO(jason): BUG!! not thread-safe */
    if (cas != req->vcas) {
        rsp->type = RSP_EXISTS;
        INCR(process_metrics, cas_exists);
        return;
    }

    /* the item might be evicted since we check */
    item_insert(it);
    rsp->type = RSP_STORED;
    INCR(process_metrics, cas_stored);

    log_verb("cas req %p processed, rsp type %d", req, rsp->type);
}

/* update item with integer value */
static item_rstatus_e
_process_delta(
        struct response *rsp, struct item *it, struct request *req, bool incr)
{
    item_rstatus_e status;
    if (incr) {
        status = item_incr(&rsp->vint, it, req->delta);
    } else {
        status = item_decr(&rsp->vint, it, req->delta);
    }
    item_release(it);
    return status;
}

static void
_process_incr(struct response *rsp, struct request *req)
{
    item_rstatus_e status;
    struct bstring *key;
    struct item *it;

    INCR(process_metrics, incr);
    key = array_first(req->keys);
    it = item_get(key, NULL, true);
    if (it != NULL) {
        status = _process_delta(rsp, it, req, true);
        if (status == ITEM_OK) {
            rsp->type = RSP_NUMERIC;
            INCR(process_metrics, incr_stored);
        } else {
            _error_rsp(rsp, status);
            INCR(process_metrics, incr_ex);
        }
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, incr_notfound);
    }

    log_verb("incr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_decr(struct response *rsp, struct request *req)
{
    item_rstatus_e status;
    struct bstring *key;
    struct item *it;

    INCR(process_metrics, decr);
    key = array_first(req->keys);
    it = item_get(key, NULL, true);
    if (it != NULL) {
        status = _process_delta(rsp, it, req, false);
        if (status == ITEM_OK) {
            rsp->type = RSP_NUMERIC;
            INCR(process_metrics, decr_stored);
        } else {
            _error_rsp(rsp, status);
            INCR(process_metrics, decr_ex);
        }
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, decr_notfound);
    }

    log_verb("decr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_append(struct response *rsp, struct request *req)
{
    log_crit("append is not supported");
}

static void
_process_prepend(struct response *rsp, struct request *req)
{
    log_crit("prepend is not supported");
}

static void
_process_flush(struct response *rsp, struct request *req)
{
    if (allow_flush) {
        INCR(process_metrics, flush);
        item_flush();
        rsp->type = RSP_OK;

        log_info("flush req %p processed, rsp type %d", req, rsp->type);
    } else {
        rsp->type = RSP_CLIENT_ERROR;
        rsp->vstr = str2bstr(CMD_ERR_MSG);
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

    case REQ_APPEND:
        _process_append(rsp, req);
        break;

    case REQ_PREPEND:
        _process_prepend(rsp, req);
        break;

    case REQ_FLUSH:
        _process_flush(rsp, req);
        break;

    default:
        rsp->type = RSP_CLIENT_ERROR;
        rsp->vstr = str2bstr(CMD_ERR_MSG);
        break;
    }
}

static inline void
_cleanup(struct request *req, struct response *rsp)
{
    struct response *nr = STAILQ_NEXT(rsp, next);

    request_reset(req);
    /* return all but the first response */
    if (nr != NULL) {
        response_return_all(&nr);
    }
    response_reset(rsp);
    req->rsp = rsp;
}

int
segcache_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_e status;
    struct request *req; /* data should be NULL or hold a req pointer */
    struct response *rsp;

    log_verb("post-read processing");

    /* deal with the stateful part: request and response */
    req = *data;
    if (req == NULL) {
        req = *data = request_borrow();
        if (req == NULL) {
            /* TODO(yao): simply return for now, better to respond with OOM */
            log_error("cannot process request: OOM");
            INCR(process_metrics, process_ex);

            return -1;
        }
    }
    rsp = (req->rsp != NULL) ? req->rsp : response_borrow();
    if (rsp == NULL) {
        request_return(&req);
        /* TODO(yao): simply return for now, better to respond with OOM */
        log_error("cannot process request: OOM");
        INCR(process_metrics, process_ex);

        return -1;
    }

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(*rbuf) > 0) {
        struct response *nr;
        int i, card;

        /* stage 1: parsing */
        log_verb("%" PRIu32 " bytes left", buf_rsize(*rbuf));

        status = parse_req(req, *rbuf);
        if (status == PARSE_EUNFIN) {
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
            return -1;
        }

        if (req->swallow) { /* skip to the end of current request */
            continue;
        }

        /* stage 2: processing- check for quit, allocate response(s), process */

        /* quit is special, no response expected */
        if (req->type == REQ_QUIT) {
            log_info("peer called quit");
            return -1;
        }

        /* find cardinality of the request and get enough response objects */
        card = array_nelem(req->keys) - 1; /* we already have one in rsp */
        if (req->type == REQ_GET || req->type == REQ_GETS) {
            /* extra response object for the "END" line after values */
            card++;
        }
        for (i = 0, nr = rsp; i < card; i++,
            STAILQ_NEXT(nr, next) = response_borrow(),
            nr = STAILQ_NEXT(nr, next)) {
            if (nr == NULL) {
                log_error("cannot acquire response: OOM");
                INCR(process_metrics, process_ex);
                _cleanup(req, rsp);
                return -1;
            }
        }

        /* actual processing */
        process_request(rsp, req);
        if (req->partial) { /* implies end of rbuf w/o complete processing */
            /* in this case, do not attempt to log or write response */
            buf_lshift(*rbuf);
            return 0;
        }

        /* stage 3: write response(s) if necessary */

        /* noreply means no need to write to buffers */
        /* TODO(jason): I don't understand the logic of card here */
        card++;
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
                    _cleanup(req, rsp);
                    /* we need to add it to rsp so that we can release here*/
                    return -1;
                }
                /* we need to add it to rsp so that we can release here*/
            }
        }

        /* logging, clean-up */
        klog_write(req, rsp);
        _cleanup(req, rsp);
    }

    return 0;
}

int
segcache_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}

int
segcache_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    struct request *req = *data;
    struct response *rsp;

    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    /* release request data & associated reserved data */
    if (req != NULL) {
        rsp = req->rsp;
        if (req->reserved != NULL) {
            struct item *it = req->reserved;
            struct bstring key = {.data = item_key(it), .len = item_nkey(it)};
            item_delete(&key);
        }
        response_return_all(&rsp);
        request_return(&req);
    }

    *data = NULL;

    return 0;
}
