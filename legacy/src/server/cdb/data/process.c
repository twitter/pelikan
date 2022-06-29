#include <limits.h>

#include "process.h"

#include "protocol/data/memcache_include.h"
#include "storage/cdb/cdb_rs.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>
#include <cc_bstring.h>

#define CDB_PROCESS_MODULE_NAME "cdb::process"

#define OVERSIZE_ERR_MSG    "oversized value, cannot be stored"
#define DELTA_ERR_MSG       "value is not a number"
#define OOM_ERR_MSG         "server is out of memory"
#define CMD_ERR_MSG         "command not supported"
#define OTHER_ERR_MSG       "unknown server error"

/* value_buf is a buffer of configurable size that processors can use by
 * rsp->vstr.data = value_buf.data. vstr.data is nulled out in response_reset
 * so the link is broken after each response */
static struct bstring value_buf;

static bool process_init = false;
static process_metrics_st *process_metrics = NULL;

static struct cdb_handle *cdb_handle = NULL;

void
process_setup(process_options_st *options, process_metrics_st *metrics, struct cdb_handle *handle)
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

    if (options->vbuf_size.val.vuint > UINT_MAX) {
        log_panic("Value for vbuf_size was too large. Must be < %ld", UINT_MAX);
    }

    value_buf.len = (uint32_t)options->vbuf_size.val.vuint;
    value_buf.data = (char *)cc_alloc(value_buf.len);

    if (value_buf.data == NULL) {
        log_panic("failed to allocate value buffer, cannot continue");
    }

    process_metrics = metrics;

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
        cdb_handle_destroy(&cdb_handle);
    }

    if (value_buf.data != NULL) {
        char *p = value_buf.data;
        bstring_init(&value_buf);
        cc_free(p);
    }

    process_metrics = NULL;
    process_init = false;
}

static bool
_get_key(struct response *rsp, struct bstring *key)
{
    /* this is a slight abuse of the bstring API. we're setting
     * the data pointer to point to the vbuf that was statically allocated
     * and we're setting the len of the buffer to the allocation size. This is
     * so that we can create a rust slice from this information. */
    rsp->vstr.data = value_buf.data;
    rsp->vstr.len = value_buf.len;

    struct bstring *vstr = cdb_get(cdb_handle, key, &(rsp->vstr));

    if (vstr != NULL) {
        rsp->type = RSP_VALUE;
        rsp->key = *key;
        rsp->flag = 0;
        rsp->vcas = 0;
        rsp->vstr = *vstr;

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
_process_invalid(struct response *rsp, struct request *req)
{
    INCR(process_metrics, invalid);
    rsp->type = RSP_CLIENT_ERROR;
    rsp->vstr = str2bstr(CMD_ERR_MSG);
    log_verb("req %p processed, rsp type %d", req, rsp->type);
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

    default:
        _process_invalid(rsp, req);
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
