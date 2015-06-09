#ifndef _BB_CODEC_H_
#define _BB_CODEC_H_

#include <protocol/memcache/bb_request.h>
#include <protocol/memcache/bb_response.h>

#include <buffer/cc_buf.h>
#include <cc_define.h>
#include <cc_metric.h>

/*          name                type            description */
#define CMD_PARSE_METRIC(ACTION)                                        \
    ACTION( cmd_total,          METRIC_COUNTER, "total # commands"     )\
    ACTION( cmd_get,            METRIC_COUNTER, "# get commands"       )\
    ACTION( cmd_gets,           METRIC_COUNTER, "# gets commands"      )\
    ACTION( cmd_delete,         METRIC_COUNTER, "# delete commands"    )\
    ACTION( cmd_set,            METRIC_COUNTER, "# set commands"       )\
    ACTION( cmd_add,            METRIC_COUNTER, "# add commands"       )\
    ACTION( cmd_replace,        METRIC_COUNTER, "# replace commands"   )\
    ACTION( cmd_cas,            METRIC_COUNTER, "# cas commands"       )\
    ACTION( cmd_append,         METRIC_COUNTER, "# append commands"    )\
    ACTION( cmd_prepend,        METRIC_COUNTER, "# prepend commands"   )\
    ACTION( cmd_incr,           METRIC_COUNTER, "# incr commands"      )\
    ACTION( cmd_decr,           METRIC_COUNTER, "# decr commands"      )\
    ACTION( cmd_stats,          METRIC_COUNTER, "# stats commands"     )\
    ACTION( cmd_quit,           METRIC_COUNTER, "# quit commands"      )

/*          name                type            description */
#define REQUEST_PROC_METRIC(ACTION)                                     \
    ACTION( request_parse,      METRIC_COUNTER, "# requests parsed"    )\
    ACTION( request_parse_ex,   METRIC_COUNTER, "# parsing error"      )\
    ACTION( request_swallow,    METRIC_COUNTER, "# lines swallowed"    )

/*          name                        type            description */
#define RESPONSE_PROC_METRIC(ACTION)                                            \
    ACTION( response_compose,           METRIC_COUNTER, "# responses composed" )\
    ACTION( response_compose_ex,        METRIC_COUNTER, "# composing error"    )

typedef struct {
    REQUEST_PROC_METRIC(METRIC_DECLARE)
    RESPONSE_PROC_METRIC(METRIC_DECLARE)
    CMD_PARSE_METRIC(METRIC_DECLARE)
} codec_metrics_st;

#define CODEC_METRIC_INIT(_metrics) do {        \
    *(_metrics) = (codec_metrics_st) {          \
        REQUEST_PROC_METRIC(METRIC_INIT)        \
        RESPONSE_PROC_METRIC(METRIC_INIT)       \
        CMD_PARSE_METRIC(METRIC_INIT) };        \
} while(0)

void codec_setup(codec_metrics_st *metrics);
void codec_teardown(void);

rstatus_t parse_swallow(struct buf *buf);
rstatus_t parse_req_hdr(struct request *req, struct buf *buf);
rstatus_t parse_req_val(struct request *req, struct buf *buf);
rstatus_t parse_req(struct request *req, struct buf *buf);

rstatus_t compose_rsp_msg(struct buf *buf, rsp_index_t idx, bool noreply);
rstatus_t compose_rsp_uint64(struct buf *buf, uint64_t val, bool noreply);
rstatus_t compose_rsp_keyval(struct buf *buf, struct bstring *key, struct bstring *val, uint32_t flag, uint64_t cas);
rstatus_t compose_rsp_stats(struct buf *buf, struct metric marr[], unsigned int nmetric);
#endif /* _BB_CODEC_H_ */
