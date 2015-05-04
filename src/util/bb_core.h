#ifndef _BB_CORE_H_
#define _BB_CORE_H_

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */

#include <cc_define.h>
#include <cc_stream.h>

/*          name                type            description */
#define CORE_METRIC(ACTION)                                         \
    ACTION( event_total,     METRIC_COUNTER, "# events returned"   )\
    ACTION( event_returns,   METRIC_COUNTER, "# events returned"    )\
    ACTION( event_read,      METRIC_COUNTER, "# core_read events"  )\
    ACTION( event_write,     METRIC_COUNTER, "# core_write events" )\
    ACTION( event_error,     METRIC_COUNTER, "# core_error events" )

struct addrinfo;

rstatus_t core_setup(struct addrinfo *ai);
void core_teardown(void);
rstatus_t core_evwait(void);

#endif /* _BB_CORE_H_ */
