#include "parse.h"

#include "request.h"
#include "response.h"
#include "token.h"

#include <cc_array.h>
#include <cc_debug.h>
#include <cc_print.h>
#include <cc_util.h>

#include <ctype.h>

#define PARSE_MODULE_NAME "protocol::resp::parse"

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

    ASSERT (el->type == ELEM_BLOB_STR);
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
    uint64_t nelem;
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

static inline parse_rstatus_e
parse_attrs(struct array *attrs, struct buf *buf)
{
    uint64_t nelem;

    parse_rstatus_e status = token_attribute_nelem(&nelem, buf);
    if (status != PARSE_OK) {
        return status;
    }
    if (nelem > attrs->nalloc) {
        log_warn("attributes map contained %" PRIu64 " > %" PRIu32 " elements", nelem, 
                attrs->nalloc);
        return PARSE_EOVERSIZE;
    }

    log_verb("parsing attributes with %" PRIu64 " key-value pair%s", nelem,
            nelem == 1 ? "" : "s");

    for (uint64_t i = 0; i < nelem; ++i) {
        struct attribute_entry entry;

        status = parse_element(&entry.key, buf);
        if (status != PARSE_OK) {
            return status;
        }

        status = parse_element(&entry.val, buf);
        if (status != PARSE_OK) {
            return status;
        }

        void *place = array_push(attrs);
        memcpy(place, &entry, sizeof(entry));
    }

    return PARSE_OK;
}

static inline parse_rstatus_e
token_is_aggregate(struct buf* buf) {
    return token_is_array(buf)
        || token_is_map(buf)
        || token_is_set(buf)
        || token_is_push_data(buf);
}

static inline parse_rstatus_e
parse_aggregate_nelem(uint64_t *nelem, struct buf *buf, element_type_e *type)
{
    /* Note: don't allow attributes since an attribute by itself isn't valid */

    if (token_is_array(buf)) {
        *type = ELEM_ARRAY;
        log_verb("parsing array header");
        return token_array_nelem(nelem, buf);
    }
    if (token_is_map(buf)) {
        *type = ELEM_MAP;
        log_verb("parsing map header");
        return token_map_nelem(nelem, buf);
    }
    if (token_is_set(buf)) {
        *type = ELEM_SET;
        log_verb("parsing set header");
        return token_set_nelem(nelem, buf);
    }
    if (token_is_push_data(buf)) {
        *type = ELEM_PUSH_DATA;
        log_verb("parsing push data header");
        return token_push_data_nelem(nelem, buf);
    }

    return PARSE_EINVALID;
}

parse_rstatus_e
parse_rsp(struct response *rsp, struct buf *buf)
{
    char *old_rpos = buf->rpos;
    parse_rstatus_e status;

    response_reset(rsp);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    if (token_is_attribute(buf)) {
        status = parse_attrs(rsp->attrs, buf);
        if (status != PARSE_OK) {
            buf->rpos = old_rpos;
            return status;
        }
    } else {
        /* no attributes is represented by an empty attrs array */
    }

    uint64_t nelem = 1;
    if (token_is_aggregate(buf)) {
        status = parse_aggregate_nelem(&nelem, buf, &rsp->type);
        if (status != PARSE_OK) {
            buf->rpos = old_rpos;
            return status;
        }

        if (nelem > rsp->token->nalloc) {
            log_debug("parse rsp: invalid # of eleents, %d > %" PRIu32, nelem, rsp->token->nalloc);
            buf->rpos = old_rpos;
            return status;
        }

        log_verb("parsing aggregate structure with %" PRIu64 " elements", nelem);
    }

    /* parse elements */
    for (uint64_t i = 0; i < nelem; ++i) {
        struct element *el = array_push(rsp->token);
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
    }

    return PARSE_OK;
}
