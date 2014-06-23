#ifndef __BB_MEMCACHE_H__
#define __BB_MEMCACHE_H__

#include <cc_mbuf.h>

#define MAX_KEY_LEN 250

struct request;

typedef void (*request_parse_t)(struct request *);

void memcache_parse_hdr(struct request *req, struct mbuf *buf);

#endif
