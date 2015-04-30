#ifndef _BB_CORE_H_
#define _BB_CORE_H_

#include <cc_define.h>
#include <cc_stream.h>

#include <netdb.h>

rstatus_t core_setup(struct addrinfo *ai);
void core_teardown(void);
rstatus_t core_evwait(void);

#endif /* _BB_CORE_H_ */
