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

static parse_rstatus_t
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
    if ((cmd.narg >= 0 && cmd.narg != narg) || narg + cmd.narg < 0) {
        log_warn("wrong number of arguments for '%.*s': %d expected, %d given",
                cmd.bstr.len, cmd.bstr.data, cmd.narg, narg);
        return PARSE_EINVALID;
    }

    return PARSE_OK;
}

parse_rstatus_t
parse_req(struct request *req, struct buf *buf)
{
    parse_rstatus_t status = PARSE_OK;
    char *old_rpos = buf->rpos;
    int32_t nelem;
    struct element *el;

    log_verb("parsing buf %p into req %p", buf, req);

    /* get number of elements in the array */
    if (!token_is_array(buf)) {
        log_debug("parse req failed: not an array");
        return PARSE_EINVALID;
    }
    status = token_array_nelem(&nelem, buf);
    if (status != PARSE_OK) {
        buf->rpos = old_rpos;
        return status;
    }
    if (nelem < 1 || nelem > req->token->nalloc) {
        log_debug("parse req: invalid array size, %d not in [1, %"PRIu32"]",
                nelem, req->token->nalloc);
        return PARSE_EINVALID;
    }

    /* parse elements */
    while (nelem > 0) {
        el = array_push(req->token);
        status = parse_element(el, buf);
        if (status != PARSE_OK) {
            log_verb("parse element returned status %d", status);
            request_reset(req);
            buf->rpos = old_rpos;
            return status;
        }
        nelem--;
    }

    status = _parse_cmd(req);
    if (status != PARSE_OK) {
        return status;
    }

    return PARSE_OK;
}
