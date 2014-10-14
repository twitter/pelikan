#ifndef _BB_PROCESS_H_
#define _BB_PROCESS_H_

#include <memcache/bb_codec.h>

extern struct glob_stats gs;
extern unsigned int nstats;

rstatus_t process_request(struct request *req, struct mbuf *buf);

#endif /* _BB_PROCESS_H_ */
