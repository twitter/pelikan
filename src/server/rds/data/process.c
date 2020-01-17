#include "process.h"

#include "protocol/data/resp_include.h"

#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_print.h>

#define RDS_PROCESS_MODULE_NAME "rds::process"

#define OVERSIZE_ERR_MSG    "oversized value, cannot be stored"
#define OOM_ERR_MSG         "server is out of memory"
#define CMD_ERR_MSG         "command not supported"
#define OTHER_ERR_MSG       "unknown server error"


typedef void (* command_fn)(struct response *, const struct request *, const
        struct command *cmd);
static command_fn command_registry[REQ_SENTINEL];

static bool process_init = false;
process_metrics_st *process_metrics = NULL;

void
process_setup(process_options_st *options, process_metrics_st *metrics)
{
    log_info("set up the %s module", RDS_PROCESS_MODULE_NAME);

    if (process_init) {
        log_warn("%s has already been setup, overwrite",
                 RDS_PROCESS_MODULE_NAME);
    }

    process_metrics = metrics;

    if (options != NULL) {
        allow_flush = option_bool(&options->allow_flush);
    }

    command_registry[REQ_LIST_CREATE] = cmd_list_create;
    command_registry[REQ_LIST_DELETE] = cmd_list_delete;
    command_registry[REQ_LIST_TRIM] = cmd_list_trim;
    command_registry[REQ_LIST_LEN] = cmd_list_len;
    command_registry[REQ_LIST_FIND] = cmd_list_find;
    command_registry[REQ_LIST_GET] = cmd_list_get;
    command_registry[REQ_LIST_INSERT] = cmd_list_insert;
    command_registry[REQ_LIST_PUSH] = cmd_list_push;

    command_registry[REQ_SARRAY_CREATE] = cmd_sarray_create;
    command_registry[REQ_SARRAY_DELETE] = cmd_sarray_delete;
    command_registry[REQ_SARRAY_TRUNCATE] = cmd_sarray_truncate;
    command_registry[REQ_SARRAY_LEN] = cmd_sarray_len;
    command_registry[REQ_SARRAY_FIND] = cmd_sarray_find;
    command_registry[REQ_SARRAY_GET] = cmd_sarray_get;
    command_registry[REQ_SARRAY_INSERT] = cmd_sarray_insert;
    command_registry[REQ_SARRAY_REMOVE] = cmd_sarray_remove;

    command_registry[REQ_SMAP_CREATE] = cmd_smap_create;
    command_registry[REQ_SMAP_DELETE] = cmd_smap_delete;
    command_registry[REQ_SMAP_TRUNCATE] = cmd_smap_truncate;
    command_registry[REQ_SMAP_LEN] = cmd_smap_len;
    command_registry[REQ_SMAP_FIND] = cmd_smap_find;
    command_registry[REQ_SMAP_GET] = cmd_smap_get;
    command_registry[REQ_SMAP_INSERT] = cmd_smap_insert;
    command_registry[REQ_SMAP_REMOVE] = cmd_smap_remove;

    command_registry[REQ_PING] = cmd_ping;

    process_init = true;
}

void
process_teardown(void)
{
    log_info("tear down the %s module", RDS_PROCESS_MODULE_NAME);
    if (!process_init) {
        log_warn("%s has never been setup", RDS_PROCESS_MODULE_NAME);
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
    struct command cmd;
    command_fn func = command_registry[req->type];
    int64_t nattrib;

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
rds_process_read(struct buf **rbuf, struct buf **wbuf, void **data)
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
rds_process_write(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-write processing");

    buf_lshift(*rbuf);
    dbuf_shrink(rbuf);
    buf_lshift(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}


int
rds_process_error(struct buf **rbuf, struct buf **wbuf, void **data)
{
    log_verb("post-error processing");

    /* normalize buffer size */
    buf_reset(*rbuf);
    dbuf_shrink(rbuf);
    buf_reset(*wbuf);
    dbuf_shrink(wbuf);

    return 0;
}
