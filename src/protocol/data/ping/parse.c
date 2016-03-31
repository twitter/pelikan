#include <protocol/data/ping/parse.h>

#include <cc_bstring.h>
#include <cc_debug.h>

#define PARSE_MODULE_NAME "protocol::ping::parse"

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

parse_rstatus_t
parse_req(struct buf *buf)
{
    log_verb("parsing buf %p", buf);

    if (buf_rsize(buf) < REQ_LEN) {
        return PARSE_EUNFIN;
    }

    if (cc_memcmp(buf->rpos, REQUEST, REQ_LEN) == 0) {
        buf->rpos += REQ_LEN;
        INCR(parse_req_metrics, request_parse);
        return PARSE_OK;
    } else { /* invalid request */
        INCR(parse_req_metrics, request_parse_ex);
        return PARSE_EOTHER;
    }
}

parse_rstatus_t
parse_rsp(struct buf *buf)
{
    log_verb("parsing buf %p", buf);

    if (buf_rsize(buf) < RSP_LEN) {
        return PARSE_EUNFIN;
    }

    if (cc_memcmp(buf->rpos, RESPONSE, RSP_LEN) == 0) {
        buf->rpos += RSP_LEN;
        INCR(parse_rsp_metrics, response_parse);
        return PARSE_OK;
    } else { /* invalid request */
        INCR(parse_rsp_metrics, response_parse_ex);
        return PARSE_EOTHER;
    }
}

