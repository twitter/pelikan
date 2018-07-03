#include "process.h"

#include "protocol/data/memcache_include.h"
#include "storage/cdb/cdb.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>

#define CDB_PROCESS_MODULE_NAME "cdb::process"

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

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;
static bool allow_flush = ALLOW_FLUSH;

static struct CDBHandle *cdb_handle = NULL;

void
process_setup(process_options_st *options, process_metrics_st *metrics, struct CDBHandle *handle)
{
    log_info("set up the %s module", CDB_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 CDB_PROCESS_MODULE_NAME);
    }

    if (handle == NULL) {
        log_panic("cdb_handle was null, cannot continue");
    }
    cdb_handle = handle;

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
    }

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", CDB_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", CDB_PROCESS_MODULE_NAME);
    }

    if (cdb_handle != NULL) {
        struct CDBHandle *p = cdb_handle;
        cdb_handle = NULL;
        cdb_handle_destroy(p);
    }

    allow_flush = false;
    process_metrics = NULL;
    process_init = false;
}

static bool
_get_key(struct response *rsp, struct bstring *key)
{
    /* this is a slight abuse of the bstring API. we're setting
     * the data pointer to point to the vbuf that was allocated on the struct
     * and we're setting the len of the buffer to the allocation size. This is
     * so that we can create a rust slice from this information. */
    rsp->vstr.data = rsp->vbuf;
    rsp->vstr.len = RSP_VAL_BUF_SIZE;

    struct bstring *vstr = cdb_get(cdb_handle, key, &(rsp->vstr));

    if (vstr != NULL) {
        rsp->type = RSP_VALUE;
        rsp->key = *key;
        rsp->flag = 0;
        rsp->vcas = 0;
        rsp->vstr.len = vstr->len;
        rsp->vstr.data = vstr->data;

        log_verb("found key at %p, location %p", key, vstr);
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
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("delete req %p processed, rsp type %d", req, rsp->type);
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
    INCR(process_metrics, set);
    INCR(process_metrics, set_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("set req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_add(struct response *rsp, struct request *req)
{
    INCR(process_metrics, add_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("add req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_replace(struct response *rsp, struct request *req)
{
    INCR(process_metrics, replace_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("replace req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_cas(struct response *rsp, struct request *req)
{
    INCR(process_metrics, cas_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("cas req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_incr(struct response *rsp, struct request *req)
{
    INCR(process_metrics, incr);
    INCR(process_metrics, incr_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("incr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_decr(struct response *rsp, struct request *req)
{
    INCR(process_metrics, decr);
    INCR(process_metrics, decr_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("decr req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_append(struct response *rsp, struct request *req)
{
    INCR(process_metrics, append_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("append req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_prepend(struct response *rsp, struct request *req)
{
    INCR(process_metrics, prepend_ex);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("prepend req %p processed, rsp type %d", req, rsp->type);
}

static void
_process_flush(struct response *rsp, struct request *req)
{
    INCR(process_metrics, flush);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_info("flush req %p processed, rsp type %d", req, rsp->type);
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
cdb_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_t status;
    struct request *req; /* data should be NULL or hold a req pointer */
    struct response *rsp;

    log_verb("post-read processing");

    /* deal with the stateful part: request and response */
    req = (*data != NULL) ? *data : request_borrow();
    if (req  == NULL) {
        /* TODO(yao): simply return for now, better to respond with OOM */
        log_error("cannot process request: OOM");
        INCR(process_metrics, process_ex);

        return -1;
    }
    rsp = (req->rsp != NULL) ? req->rsp : response_borrow();
    if (rsp  == NULL) {
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
cdb_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}


int
cdb_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
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
        request_return(&req);
        response_return_all(&rsp);
    }

    return 0;
}
