#ifndef _BB_PROCESS_H_
#define _BB_PROCESS_H_

#include <cc_define.h>

struct request;
struct buf;

rstatus_t process_request(struct request *req, struct buf *buf);

#endif /* _BB_PROCESS_H_ */
