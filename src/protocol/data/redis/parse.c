#include "parse.h"

#include "request.h"
#include "response.h"
#include "token.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>

#define PARSE_MODULE_NAME "protocol::redis::parse"

static bool parse_init = false;
static parse_req_metrics_st *parse_req_metrics = NULL;
static parse_rsp_metrics_st *parse_rsp_metrics = NULL;

void
parse_setup(parse_req_metrics_st *req, parse_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", PARSE_MODULE_NAME);

    if (parse_init) {
        log_warn("%s has already been setup, overwrite", PARSE_MODULE_NAME);
    }

    parse_req_metrics = req;
    parse_rsp_metrics = rsp;
    parse_init = true;
}

void
parse_teardown(void)
{
    log_info("tear down the %s module", PARSE_MODULE_NAME);

    if (!parse_init) {
        log_warn("%s has never been setup", PARSE_MODULE_NAME);
    }

    parse_req_metrics = NULL;
    parse_rsp_metrics = NULL;
    parse_init = false;
}

static parse_rstatus_e
_parse_cmd(struct request *req)
{
    cmd_type_e type;
    struct command cmd;
    struct element *el;
    int narg;

    ASSERT(req != NULL);

    /* check verb */
    type = REQ_UNKNOWN;
    el = array_first(req->token);

    ASSERT (el->type == ELEM_BULK);
    while (++type < REQ_SENTINEL &&
            bstring_compare(&command_table[type].bstr, &el->bstr) != 0) {}
    if (type == REQ_SENTINEL) {
        log_warn("unrecognized command detected: %.*s", el->bstr.len,
                el->bstr.data);
        return PARSE_EINVALID;
    }
    req->type = type;

    /* check narg */
    cmd = command_table[type];
    narg = req->token->nelem;
    if (narg < cmd.narg || narg > (cmd.narg + cmd.nopt)) {
        log_warn("wrong # of arguments for '%.*s': %d+[%d] expected, %d given",
                cmd.bstr.len, cmd.bstr.data, cmd.narg, cmd.nopt, narg);
        return PARSE_EINVALID;
    }

    return PARSE_OK;
}


parse_rstatus_e
parse_req(struct request *req, struct buf *buf)
{
    parse_rstatus_e status = PARSE_OK;
    char *old_rpos = buf->rpos;
    int64_t nelem;
    struct element *el;

    log_verb("parsing buf %p into req %p", buf, req);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    /* get number of elements in the array */
    if (!token_is_array(buf)) {
        log_debug("parse req failed: not an array");
        return PARSE_EINVALID;
    }
    status = token_array_nelem(&nelem, buf);
    if (status != PARSE_OK) {
        log_verb("getting array size returns status %d", status);
        buf->rpos = old_rpos;
        return status;
    } else {
        log_verb("array size is %"PRId64, nelem);
    }

    if (nelem < 1 || nelem > req->token->nalloc) {
        log_debug("parse req: invalid array size, %d not in [1, %"PRIu32"]",
                nelem, req->token->nalloc);
        return PARSE_EINVALID;
    }


    /* parse elements */
    while (nelem > 0) {
        if (buf_rsize(buf) == 0) {
            buf->rpos = old_rpos;
            return PARSE_EUNFIN;
        }
        el = array_push(req->token);
        status = parse_element(el, buf);
        log_verb("parse element returned status %d", status);
        if (status != PARSE_OK) {
            request_reset(req);
            buf->rpos = old_rpos;
            return status;
        }
        nelem--;
    }

    status = _parse_cmd(req);
    log_verb("parse command returned status %d", status);
    if (status != PARSE_OK) {
        buf->rpos = old_rpos;
        return status;
    }

    return PARSE_OK;
}

parse_rstatus_e
parse_rsp(struct response *rsp, struct buf *buf)
{
    parse_rstatus_e status = PARSE_OK;
    char *old_rpos = buf->rpos;
    int64_t nelem = 1;
    struct element *el;

    ASSERT(rsp->type == ELEM_UNKNOWN);

    log_verb("parsing buf %p into rsp %p", buf, rsp);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    if (token_is_array(buf)) {
        status = token_array_nelem(&nelem, buf);
        if (status != PARSE_OK) {
            buf->rpos = old_rpos;
            return status;
        }
        rsp->type = ELEM_ARRAY;
        if (nelem > rsp->token->nalloc) {
            log_debug("parse rsp: invalid # of elements, %d > %"PRIu32, nelem,
                 rsp->token->nalloc);
            return PARSE_EOVERSIZE;
        }
        if (nelem < 0) {
            rsp->nil = true;
            return PARSE_OK;
        }
    }

    /* parse elements */
    while (nelem > 0) {
        el = array_push(rsp->token);
        status = parse_element(el, buf);
        if (status != PARSE_OK) {
            log_verb("parse element returned status %d", status);
            response_reset(rsp);
            buf->rpos = old_rpos;
            return status;
        }
        if (rsp->type == ELEM_UNKNOWN) {
            rsp->type = el->type;
        }
        nelem--;
    }

    return PARSE_OK;
}
