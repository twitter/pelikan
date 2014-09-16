#ifndef _BB_codec_H_
#define _BB_codec_H_

#include <memcache/bb_request.h>

#include <cc_define.h>
#include <cc_mbuf.h>

rstatus_t parse_req_hdr(struct request *req, struct mbuf *buf);

#endif /* _BB_CODEC_H_ */
