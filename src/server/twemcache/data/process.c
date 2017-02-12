#include "process.h"

#include "protocol/data/memcache_include.h"
#include "storage/slab/slab.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>

#define TWEMCACHE_PROCESS_MODULE_NAME "twemcache::process"

#define OVERSIZE_ERR_MSG    "oversized value, cannot be stored"
#define DELTA_ERR_MSG       "value is not a number"
#define OOM_ERR_MSG         "server is out of memory"
#define CMD_ERR_MSG         "command not supported"
#define OTHER_ERR_MSG       "unknown server error"

typedef enum put_rstatus {
    PUT_OK,
    PUT_PARTIAL,
    PUT_ERROR,
} put_rstatus_t;

/* the data pointer in the process functions is of type `struct data **' */
struct data {
    struct request *req;
    struct response *rsp;
};

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;
static bool allow_flush = ALLOW_FLUSH;

void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", TWEMCACHE_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 TWEMCACHE_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
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

    allow_flush = false;
    process_metrics = NULL;
    process_init = false;
}


static bool
_get_key(struct response *rsp, struct bstring *key)
{
    struct item *it;

    it = item_get(key);
    if (it != NULL) {
        rsp->type = RSP_VALUE;
        rsp->key = *key;
        rsp->flag = item_flag(it);
        rsp->vcas = item_get_cas(it);
        rsp->vstr.len = it->vlen;
        rsp->vstr.data = item_data(it);

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
    if (item_delete(array_first(req->keys))) {
        rsp->type = RSP_DELETED;
        INCR(process_metrics, delete_deleted);
    } else {
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, delete_notfound);
    }

    log_verb("delete req %p processed, rsp type %d", req, rsp->type);
}


static void
_error_rsp(struct response *rsp, item_rstatus_t status)
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
static put_rstatus_t
_put(item_rstatus_t *istatus, struct request *req)
{
    put_rstatus_t status;
    struct item *it = NULL;

    *istatus = ITEM_OK;
    if (req->first) { /* self-contained req */
        struct bstring *key = array_first(req->keys);
        *istatus = item_reserve(&it, key, &req->vstr, req->vlen, req->flag,
                time_reltime(req->expiry));
        req->first = false;
        req->reserved = it;
    } else { /* backfill reserved item */
        item_backfill(req->reserved, &req->vstr);
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
    put_rstatus_t status;
    item_rstatus_t istatus;
    struct item *it;
    struct bstring key;

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
    key = (struct bstring){it->klen, item_key(it)};
    item_insert(it, &key);
    rsp->type = RSP_STORED;
    INCR(process_metrics, set_stored);

    log_verb("set req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_add(struct response *rsp, struct request *req)
{
    put_rstatus_t status;
    item_rstatus_t istatus;
    struct item *it;
    struct bstring key;

    status = _put(&istatus, req);
    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        _error_rsp(rsp, istatus);
        INCR(process_metrics, add_ex);

        return;
    }

    /* PUT_OK, meaning we have an item reserved, i.e. req->reserved != NULL */
    INCR(process_metrics, add);
    it = (struct item *)req->reserved;
    key = (struct bstring){it->klen, item_key(it)};
    if (item_get(&key) != NULL) {
        item_release((struct item **)&req->reserved);
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, add_notstored);
    } else {
        item_insert(it, &key);
        rsp->type = RSP_STORED;
        INCR(process_metrics, add_stored);
    }

    log_verb("add req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_replace(struct response *rsp, struct request *req)
{
    put_rstatus_t status;
    item_rstatus_t istatus;
    struct item *it = NULL;
    struct bstring key;

    status = _put(&istatus, req);
    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        _error_rsp(rsp, istatus);
        INCR(process_metrics, replace_ex);

        return;
    }

    /* PUT_OK, meaning we have an item reserved, i.e. req->reserved != NULL */
    INCR(process_metrics, replace);
    it = (struct item *)req->reserved;
    key = (struct bstring){it->klen, item_key(it)};
    if (item_get(&key) != NULL) {
        item_insert(it, &key);
        rsp->type = RSP_STORED;
        INCR(process_metrics, replace_stored);
    } else {
        item_release((struct item **)&req->reserved);
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, replace_notstored);
    }

    log_verb("replace req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_cas(struct response *rsp, struct request *req)
{
    put_rstatus_t status;
    item_rstatus_t istatus;
    struct item *it, *oit;
    struct bstring key;

    status = _put(&istatus, req);
    if (status == PUT_PARTIAL) {
        return;
    }
    if (status == PUT_ERROR) {
        _error_rsp(rsp, istatus);
        INCR(process_metrics, cas_ex);

        return;
    }

    /* PUT_OK, meaning we have an item reserved, i.e. req->reserved != NULL */
    it = (struct item *)req->reserved;
    key = (struct bstring){it->klen, item_key(it)};
    oit = item_get(&key);
    if (oit == NULL) {
        item_release((struct item **)&req->reserved);
        rsp->type = RSP_NOT_FOUND;
        INCR(process_metrics, cas_notfound);
    } else {
        if (item_get_cas(oit) != req->vcas) {
            item_release((struct item **)&req->reserved);
            rsp->type = RSP_EXISTS;
            INCR(process_metrics, cas_exists);
        } else {
            item_insert(it, &key);
            rsp->type = RSP_STORED;
            INCR(process_metrics, cas_stored);
        }
    }

    log_verb("cas req %p processed, rsp type %d", req, rsp->type);
}

/* get integer value of it */

/* update item with integer value */
static item_rstatus_t
_process_delta(struct response *rsp, struct item *it, struct request *req,
        struct bstring *key, bool incr)
{
    item_rstatus_t status;
    uint32_t dataflag;
    uint64_t vint;
    struct bstring nval;
    char buf[CC_UINT64_MAXLEN];

    status = item_atou64(&vint, it);
    if (status != ITEM_OK) {
        return status;
    }

    if (incr) {
        vint += req->delta;
    } else {
        if (vint < req->delta) {
            vint = 0;
        } else {
            vint -= req->delta;
        }
    }

    rsp->vint = vint;
    nval.len = cc_print_uint64_unsafe(buf, vint);
    nval.data = buf;
    if (item_slabid(it->klen, nval.len) == it->id) {
        item_update(it, &nval);
        return ITEM_OK;
    }

    dataflag = it->dataflag;
    status = item_reserve(&it, key, &nval, nval.len, dataflag,
            it->expire_at);
    if (status == ITEM_OK) {
        item_insert(it, key);
    }

    return status;
}

static void
_process_incr(struct response *rsp, struct request *req)
{
    item_rstatus_t status;
    struct bstring *key;
    struct item *it;

    INCR(process_metrics, incr);
    key = array_first(req->keys);
    it = item_get(key);
    if (it != NULL) {
        status = _process_delta(rsp, it, req, key, true);
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
    item_rstatus_t status;
    struct bstring *key;
    struct item *it;

    INCR(process_metrics, decr);
    key = array_first(req->keys);
    it = item_get(key);
    if (it != NULL) {
        status = _process_delta(rsp, it, req, key, false);
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
    item_rstatus_t status;
    struct bstring *key;
    struct item *it;

    key = array_first(req->keys);
    it = item_get(key);
    if (it == NULL) {
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, append_notstored);
    } else {
        if (req->partial) { /* reject incomplete append requests */
            status = ITEM_EOVERSIZED;
        } else {
            status = item_annex(it, key, &(req->vstr), true);
        }
        if (status == ITEM_OK) {
            rsp->type = RSP_STORED;
            INCR(process_metrics, append_stored);
        } else {
            _error_rsp(rsp, status);
            INCR(process_metrics, append_ex);
        }
    }

    log_verb("append req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_prepend(struct response *rsp, struct request *req)
{
    item_rstatus_t status;
    struct bstring *key;
    struct item *it;

    key = array_first(req->keys);
    it = item_get(key);
    if (it == NULL) {
        rsp->type = RSP_NOT_STORED;
        INCR(process_metrics, prepend_notstored);
    } else {
        if (req->partial) { /* reject incomplete prepend requests */
            status = ITEM_EOVERSIZED;
        } else {
            status = item_annex(it, key, &(req->vstr), false);
        }
        if (status == ITEM_OK) {
            rsp->type = RSP_STORED;
            INCR(process_metrics, prepend_stored);
        } else {
            _error_rsp(rsp, status);
            INCR(process_metrics, prepend_ex);
        }
    }

    log_verb("prepend req %p processed, rsp type %d", req, rsp->type);
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
}

static inline struct data *
_data_create(void)
{
    struct data *data;
    struct request *req;
    struct response *rsp;

    req = request_borrow();
    rsp = response_borrow();
    data = cc_alloc(sizeof(struct data));

    if (req == NULL || rsp == NULL || data == NULL) {
        request_return(&req);
        response_return(&rsp);
        cc_free(data); /* cc_free(data) is a macro that sets data to NULL */
    } else {
        data->req = req;
        data->rsp = rsp;
    }

    return data;
}

int
twemcache_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_t status;
    struct data *state;
    struct request *req;
    struct response *rsp;

    log_verb("post-read processing");

    /* deal with the stateful part: request and response */
    if (*data == NULL) {
        if ((*data = _data_create()) == NULL) {
            /* TODO(yao): simply return for now, better to respond with OOM */
            log_error("cannot process request: OOM");
            INCR(process_metrics, process_ex);

            return -1;
        }
    }
    state = (struct data *)*data;
    req = state->req;
    rsp = state->rsp;

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(*rbuf) > 0) {
        struct response *nr;
        int i, card;

        /* stage 1: parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(*rbuf));

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
        for (i = 0, nr = rsp;
             i < card;
             i++, STAILQ_NEXT(nr, next) = response_borrow(), nr =
                STAILQ_NEXT(nr, next)) {
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
                    return -1;
                }
            }
        }

        /* logging, clean-up */
        klog_write(req, rsp);
        _cleanup(req, rsp);
    }

    return 0;
}


int
twemcache_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(rbuf);
    dbuf_shrink(wbuf);

    return 0;
}


int
twemcache_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    struct data *state = (struct data *)*data;
    struct request *req;
    struct response *rsp;

    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    /* release request data & associated reserved data */
    if (*data != NULL) {
        req = state->req;
        rsp = state->rsp;
        if (req->reserved != NULL) {
            item_release((struct item **)&req->reserved);
        }
        request_return(&req);
        response_return_all(&rsp);
        cc_free(state);
    }

    return 0;
}
