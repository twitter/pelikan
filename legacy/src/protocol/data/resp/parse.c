#include "parse.h"

#include "request.h"
#include "response.h"
#include "token.h"
#include "attribute.h"

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
    el = array_get(req->token, CMD_OFFSET);

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
    narg = req->token->nelem - 1;
    if (narg < cmd.narg || narg > (cmd.narg + cmd.nopt)) {
        log_warn("wrong # of arguments for '%.*s': %d+[%d] expected, %d given",
                cmd.bstr.len, cmd.bstr.data, cmd.narg, cmd.nopt, narg);
        return PARSE_EINVALID;
    }

    return PARSE_OK;
}

static parse_rstatus_e
_parse_range(struct array *token, struct buf *buf, int64_t nelem)
{
    parse_rstatus_e status;
    struct element *el;

    while (nelem > 0) {
        if (buf_rsize(buf) == 0) {
            return PARSE_EUNFIN;
        }
        el = array_push(token);
        status = parse_element(el, buf);
        log_verb("parse element returned status %d", status);
        if (status != PARSE_OK) {
            return status;
        }
        nelem--;
    }

    return PARSE_OK;
}


/* true if attribute is valid, false otherwise */
static attrib_type_e
_lookup_attrib_key(struct element *key)
{
    attrib_type_e type = ATTRIB_UNKNOWN;

    if (key->type != ELEM_STR) { /* key has to be a simple string */
        log_warn("attribute key must be simple string, not type %d", key->type);

        return ATTRIB_UNKNOWN;
    }

    for (; type < ATTRIB_SENTINEL; ++type) {
        if (bstring_compare(&attrib_table[type], &key->bstr) == 0) {
            break;
        }
    }
    if (type == ATTRIB_SENTINEL) {
        log_warn("unrecognized attribute: %.*s", key->bstr.len, key->bstr.data);

        return ATTRIB_SENTINEL;
    }

    return type;
}

static void
_parse_req_attrib(struct request *req, struct element *key, struct element *val)
{
    switch (_lookup_attrib_key(key)) {
    case ATTRIB_TTL:
        if (val->type != ELEM_INT) {
            log_warn("attribute ttl has value type int, %d found", val->type);

            return;
        }
        req->ttl = val->num;
        log_verb("request provides attribute 'ttl', value is %"PRIi64, req->ttl);
        break;

    case ATTRIB_FLAG:
        if (val->type != ELEM_INT || val->num > UINT32_MAX || val->num < 0) {
            log_warn("attribute ttl has invalid type or value");

            return;
        }
        req->flag = val->num;
        log_verb("request provides attribute 'flag', value is %"PRIi64, req->flag);
        break;

    default:
        /* do nothing, including unknown or invalid */
        break;
    }
}

parse_rstatus_e
parse_req(struct request *req, struct buf *buf)
{
    parse_rstatus_e status = PARSE_OK;
    char *old_rpos = buf->rpos;
    struct element *el, attrib, key, val;
    uint32_t cap = array_nalloc(req->token);

    ASSERT(cap > 1);

    log_verb("parsing buf %p into req %p", buf, req);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    /* parse attributes if present */
    if (token_is_attrib(buf)) {
        status = parse_element(&attrib, buf);
        if (status != PARSE_OK) {
            goto error;
        }
        for (;attrib.num > 0; attrib.num--) {
            status = parse_element(&key, buf);
            if (status != PARSE_OK) {
                goto error;
            }
            status = parse_element(&val, buf);
            if (status != PARSE_OK) {
                goto error;
            }
            _parse_req_attrib(req, &key, &val);
        }
    }

    cap--; /* we will have at least 2 slots here */
    el = array_push(req->token);
    status = parse_element(el, buf);
    if (status != PARSE_OK || el->num < 1) {
        goto error;
    }

    if (el->type != ELEM_ARRAY) {
        log_debug("parse req failed: not an array");
        goto error;
    }

    status = _parse_range(req->token, buf, el->num);
    if (status != PARSE_OK) {
        goto error;
    }

    status = _parse_cmd(req);
    if (status != PARSE_OK) {
        goto error;
    }

    return PARSE_OK;

error:
    request_reset(req);
    buf->rpos = old_rpos;
    return status;
}


static void
_parse_rsp_attrib(struct response *rsp, struct element *key, struct element *val)
{
    switch (_lookup_attrib_key(key)) {
    case ATTRIB_TTL:
        if (val->type != ELEM_INT) {
            log_warn("attribute ttl has value type int, %d found", val->type);

            return;
        }
        rsp->ttl = val->num;
        log_verb("request provides attribute 'ttl', value is %"PRIi64, rsp->ttl);
        break;

    case ATTRIB_FLAG:
        if (val->type != ELEM_INT || val->num > UINT32_MAX || val->num < 0) {
            log_warn("attribute ttl has invalid type or value");

            return;
        }
        rsp->flag = val->num;
        log_verb("request provides attribute 'flag', value is %"PRIi64, rsp->flag);
        break;

    default:
        /* do nothing, including unknown or invalid */
        break;
    }
}

parse_rstatus_e
parse_rsp(struct response *rsp, struct buf *buf)
{
    parse_rstatus_e status = PARSE_OK;
    char *old_rpos = buf->rpos;
    int64_t nelem = 1;
    struct element *el, attrib, key, val;
    uint32_t cap = array_nalloc(rsp->token);

    ASSERT(cap  > 0);
    ASSERT(rsp->type == ELEM_UNKNOWN);

    log_verb("parsing buf %p into rsp %p", buf, rsp);

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    /* parse attributes if present */
    if (token_is_attrib(buf)) {
        status = parse_element(&attrib, buf);
        if (status != PARSE_OK) {
            goto error;
        }
        for (;attrib.num > 0; attrib.num--) {
            status = parse_element(&key, buf);
            if (status != PARSE_OK) {
                goto error;
            }
            status = parse_element(&val, buf);
            if (status != PARSE_OK) {
                goto error;
            }
            _parse_rsp_attrib(rsp, &key, &val);
        }
    }

    if (buf_rsize(buf) == 0) {
        return PARSE_EUNFIN;
    }

    if (token_is_array(buf)) {
        rsp->type = ELEM_ARRAY;
        cap--;
        el = array_push(rsp->token);
        status = parse_element(el, buf);
        if (status != PARSE_OK) {
            goto error;
        }
        nelem = el->num;
        if (nelem < 0) {
            rsp->nil = true;

            return PARSE_OK;
        }
    }

    status = _parse_range(rsp->token, buf, nelem);
    if (status != PARSE_OK) {
        goto error;
    }

    /* assign rsp type based on first non-attribute element */
    if (rsp->type == ELEM_UNKNOWN) {
        rsp->type = ((struct element *)array_first(rsp->token))->type;
    }

    return PARSE_OK;

error:
    response_reset(rsp);
    buf->rpos = old_rpos;
    return status;
}
