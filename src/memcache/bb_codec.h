#ifndef _BB_CODEC_H_
#define _BB_CODEC_H_

#include <memcache/bb_request.h>
#include <memcache/bb_response.h>

#include <cc_define.h>
#include <cc_mbuf.h>

rstatus_t parse_req_hdr(struct request *req, struct mbuf *buf);

rstatus_t compose_rsp_msg(struct mbuf *buf, rsp_index_t idx, bool noreply);
rstatus_t compose_rsp_uint64(struct mbuf *buf, uint64_t val, bool noreply);
rstatus_t compose_rsp_bstring(struct mbuf *buf, struct bstring *str, bool noreply);
rstatus_t compose_rsp_keyval(struct mbuf *buf, struct bstring *key, struct bstring *val, uint32_t flag, uint64_t cas);

#endif /* _BB_CODEC_H_ */
