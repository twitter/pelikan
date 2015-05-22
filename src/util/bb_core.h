#ifndef _BB_CORE_H_
#define _BB_CORE_H_

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include <util/bb_core_server.h>
#include <util/bb_core_worker.h>

#include <cc_channel.h>
#include <cc_define.h>

struct addrinfo;

rstatus_t core_setup(struct addrinfo *ai, uint32_t max_conns);
void core_teardown(void);
void core_run(void);

#endif /* _BB_CORE_H_ */
