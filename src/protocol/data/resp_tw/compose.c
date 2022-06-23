#include "compose.h"

#include "request.h"
#include "response.h"
#include "token.h"

#include <cc_debug.h>
#include <cc_print.h>

#define COMPOSE_MODULE_NAME "protocol::resp_tw::compose"

static bool compose_init = false;
static compose_req_metrics_st *compose_req_metrics = NULL;
static compose_rsp_metrics_st *compose_rsp_metrics = NULL;

void
compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", COMPOSE_MODULE_NAME);

    if (compose_init) {
        log_warn("%s has already been set up, overwrite", COMPOSE_MODULE_NAME);
    }

    compose_req_metrics = req;
    compose_rsp_metrics = rsp;

    compose_init = true;
}

void
compose_teardown(void)
{
    log_info("tear down the %s module", COMPOSE_MODULE_NAME);

    if (!compose_init) {
        log_warn("%s has never been set up", COMPOSE_MODULE_NAME);
    }

    compose_req_metrics = NULL;
    compose_rsp_metrics = NULL;
    compose_init = false;
}

int
compose_req(struct buf **buf, struct request *req)
{
    int n = compose_array_header(buf, req->token->nelem);
    if (n < 0) {
        return n;
    }

    for (size_t i = 0; i < req->token->nelem; i++) {
        int ret = compose_element(buf, array_get(req->token, i));
        if (ret < 0) {
            return ret;
        }
        n += ret;
    }

    return n;
}

static inline bool 
is_aggregate(element_type_e type) {
    switch (type) {
    case ELEM_ARRAY:
    case ELEM_ATTRIBUTES:
    case ELEM_MAP:
    case ELEM_SET:
    case ELEM_PUSH_DATA:
        return true;
    default:
        return false;
    }
}

static inline int
compose_aggregate_header(struct buf **buf, struct response *rsp)
{
    switch (rsp->type) {
    case ELEM_ARRAY:
        return compose_array_header(buf, rsp->token->nelem);
    case ELEM_MAP:
        return compose_map_header(buf, rsp->token->nelem);
    case ELEM_SET:
        return compose_set_header(buf, rsp->token->nelem);
    case ELEM_PUSH_DATA:
        return compose_push_data_header(buf, rsp->token->nelem);
    case ELEM_ATTRIBUTES:
        /* Attributes must be before another value */
        return COMPOSE_ENOTSUPPORTED;
    default:
        NOT_REACHED();
        return COMPOSE_EOTHER;
    }
}

static inline int
compose_attrs(struct buf **buf, struct array *attrs)
{
    int n = compose_attribute_header(buf, attrs->nelem);
    if (n < 0) {
        return n;
    }

    for (size_t i = 0; i < attrs->nelem; ++i) {
        struct attribute_entry *entry = array_get(attrs, i);

        int ret = compose_element(buf, &entry->key);
        if (ret < 0) {
            return ret;
        }
        n += ret;

        ret = compose_element(buf, &entry->val);
        if (ret < 0) {
            return ret;
        }
        n += ret;
    }

    return n;
}

int
compose_rsp(struct buf **buf, struct response *rsp)
{
    int n = 0;

    if (rsp->attrs->nelem > 0) {
        int ret = compose_attrs(buf, rsp->attrs);
        if (ret < 0) {
            return ret;
        }
        n += ret;
    }

    if (is_aggregate(rsp->type)) {
        int ret = compose_aggregate_header(buf, rsp);
        if (ret < 0) {
            return ret;
        }
        n += ret;
    }

    for (size_t i = 0; i < rsp->token->nelem; ++i) {
        int ret = compose_element(buf, array_get(rsp->token, i));
        if (ret < 0) {
            return ret;
        }

        n += ret;
    }

    return n;
}