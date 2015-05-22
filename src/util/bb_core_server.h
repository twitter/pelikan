#ifndef _BB_CORE_SERVER_H_
#define _BB_CORE_SERVER_H_

#include <cc_define.h>

/*          name                    type            description */
#define CORE_SERVER_METRIC(ACTION)                                                   \
    ACTION( server_event_total,     METRIC_COUNTER, "# server events returned"      )\
    ACTION( server_event_loop,      METRIC_COUNTER, "# server event loops returned" )\
    ACTION( server_event_read,      METRIC_COUNTER, "# server core_read events"     )\
    ACTION( server_event_write,     METRIC_COUNTER, "# server core_write events"    )\
    ACTION( server_event_error,     METRIC_COUNTER, "# server core_error events"    )

struct addrinfo;

rstatus_t core_server_setup(struct addrinfo *ai);
void core_server_teardown(void);
void core_server_evloop(void);

#endif /* _BB_CORE_SERVER_H_ */
