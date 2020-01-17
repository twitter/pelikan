#include "process.h"

#include "protocol/data/resp_include.h"

#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_print.h>

#define SLIMRDS_PROCESS_MODULE_NAME "slimrds::process"

#define OVERSIZE_ERR_MSG    "oversized value, cannot be stored"
#define OOM_ERR_MSG         "server is out of memory"
#define CMD_ERR_MSG         "command not supported"
#define OTHER_ERR_MSG       "unknown server error"


typedef void (* command_fn)(struct response *, struct request *, struct command *cmd);
static command_fn command_registry[REQ_SENTINEL];

static bool process_init = false;
process_metrics_st *process_metrics = NULL;

void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", SLIMRDS_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 SLIMRDS_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
    }

    command_registry[REQ_PING] = cmd_ping;

    command_registry[REQ_BITMAP_DELETE] = cmd_bitmap_delete;
    command_registry[REQ_BITMAP_CREATE] = cmd_bitmap_create;
    command_registry[REQ_BITMAP_SET] = cmd_bitmap_set;
    command_registry[REQ_BITMAP_GET] = cmd_bitmap_get;

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", SLIMRDS_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", SLIMRDS_PROCESS_MODULE_NAME);
    }

    command_registry[REQ_PING] = cmd_ping;

    allow_flush = ALLOW_FLUSH;
    process_metrics = NULL;
    process_init = false;
}


static void
_process_attribute(struct request *req, struct element *key, struct element *val)
{
    attrib_type_e type = ATTRIB_UNKNOWN;

    /* treat this as it cannot fail, skip anything that is not recognized or has
     * invalid value so the program will proceed to the request main itself.
     */
    if (key->type != ELEM_STR) { /* key has to be a simple string */
        log_warn("attribute key must be simple string, not type %d", key->type);

        return;
    }
    while (++type < ATTRIB_SENTINEL &&
            bstring_compare(&attrib_table[type], &key->bstr) != 0) {}
    if (type == REQ_SENTINEL) {
        log_warn("unrecognized attribute: %.*s", key->bstr.len, key->bstr.data);

        return;
    }

    if (type == ATTRIB_TTL) {
        if (val->type != ELEM_INT) {
            log_warn("attribute ttl has value type int, %d found", val->type);

            return;
        }
        req->ttl = val->num;

        /* TODO(yao): determine what's valid value range for TTL */
        log_verb("request provides attribute 'ttl', value is %"PRIi64, req->ttl);
    }

    /* TODO(yao): softTTL */
}


void
process_request(struct response *rsp, struct request *req)
{
    int64_t nattrib;
    struct command cmd;
    command_fn func = command_registry[req->type];

    log_verb("processing req %p, write rsp to %p", req, rsp);
    INCR(process_metrics, process_req);

    if (func == NULL) {
        struct element *reply = (struct element *)array_push(rsp->token);
        log_warn("command is recognized but not implemented");

        rsp->type = reply->type = ELEM_ERR;
        reply->bstr = str2bstr(RSP_ERR_NOSUPPORT);
        INCR(process_metrics, process_ex);

        return;
    }

    if (req->offset > 0) { /* attributes are present */
        nattrib = ((struct element *)array_first(req->token))->num;
        for (int64_t i = 1; i < nattrib * 2 + 1; i += 2) {
            _process_attribute(req, (struct element *)array_get(req->token, i),
                    (struct element *)array_get(req->token, i + 1));
        }
    }

    cmd = command_table[req->type];
    cmd.nopt = ((struct element *)array_get(req->token, req->offset))->num -
        cmd.narg;

    log_verb("processing command '%.*s' with %d optional arguments",
            cmd.bstr.len, cmd.bstr.data, cmd.nopt);
    func(rsp, req, &cmd);
}

int
slimrds_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
{
    parse_rstatus_e status;
    struct request *req; /* data should be NULL or hold a req pointer */
    struct response *rsp;

    req = request_borrow();
    rsp = response_borrow();
    if (req == NULL || rsp == NULL) {
        goto error;
    }

    /* keep parse-process-compose until running out of data in rbuf */
    while (buf_rsize(*rbuf) > 0) {
        request_reset(req);
        response_reset(rsp);

        /* stage 1: parsing */
        log_verb("%"PRIu32" bytes left", buf_rsize(*rbuf));

        status = parse_req(req, *rbuf);
        if (status == PARSE_EUNFIN) {
            buf_lshift(*rbuf);
            goto done;
        }
        if (status != PARSE_OK) {
            /* parsing errors are all client errors, since we don't know
             * how to recover from client errors in this condition (we do not
             * have a valid request so we don't know where the invalid request
             * ends), we should close the connection
             */
            log_warn("illegal request received, status: %d", status);
            INCR(process_metrics, process_ex);
            INCR(process_metrics, process_client_ex);
            goto error;
        }

        /* stage 2: processing- check for quit, allocate response(s), process */

        /* quit is special, no response expected */
        if (req->type == REQ_QUIT) {
            log_info("peer called quit");
            goto error;
        }

        /* actual processing */
        process_request(rsp, req);

        /* stage 3: write response(s) if necessary */

        /* noreply means no need to write to buffers */
        if (compose_rsp(wbuf, rsp) < 0) {
            log_error("composing rsp erred");
            INCR(process_metrics, process_ex);
            INCR(process_metrics, process_server_ex);
            goto error;
        }

        /* logging, clean-up */
    }

done:
    request_return(&req);
    response_return(&rsp);

    return 0;

error:
    request_return(&req);
    response_return(&rsp);

    return -1;
}


int
slimrds_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}


int
slimrds_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}
