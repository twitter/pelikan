#include "compose.h"

#include "request.h"
#include "response.h"
#include "token.h"

#include <cc_debug.h>
#include <cc_print.h>

#define COMPOSE_MODULE_NAME "protocol::redis::compose"

static bool compose_init = false;
static compose_req_metrics_st *compose_req_metrics = NULL;
static compose_rsp_metrics_st *compose_rsp_metrics = NULL;

void
compose_setup(compose_req_metrics_st *req, compose_rsp_metrics_st *rsp)
{
    log_info("set up the %s module", COMPOSE_MODULE_NAME);

    if (compose_init) {
        log_warn("%s has already been setup, overwrite", COMPOSE_MODULE_NAME);
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
        log_warn("%s has never been setup", COMPOSE_MODULE_NAME);
    }
    compose_req_metrics = NULL;
    compose_rsp_metrics = NULL;
    compose_init = false;
}

int
compose_req(struct buf **buf, struct request *req)
{
    int n;

    n = compose_array_header(buf, req->token->nelem);
    if (n < 0) {
        return n;
    }

    for (int i = 0; i < req->token->nelem; i++) {
        int ret;

        ret = compose_element(buf, array_get(req->token, i));
        if (ret < 0) {
            return ret;
        } else {
            n += ret;
        }
    }

    return n;
}

int
compose_rsp(struct buf **buf, struct response *rsp)
{
    int n = 0;

    if (rsp->type == ELEM_ARRAY) {
        n = compose_array_header(buf, rsp->token->nelem);
        if (n < 0) {
            return n;
        }
    }

    for (int i = 0; i < rsp->token->nelem; i++) {
        int ret;

        ret = compose_element(buf, array_get(rsp->token, i));
        if (ret < 0) {
            return ret;
        } else {
            n += ret;
        }
    }

    return n;
}
