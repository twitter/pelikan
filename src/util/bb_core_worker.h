#ifndef _BB_CORE_WORKER_H_
#define _BB_CORE_WORKER_H_

#include <cc_define.h>

/*          name                    type            description */
#define CORE_WORKER_METRIC(ACTION)                                                   \
    ACTION( worker_event_total,     METRIC_COUNTER, "# worker events returned"      )\
    ACTION( worker_event_loop,      METRIC_COUNTER, "# worker event loops returned" )\
    ACTION( worker_event_read,      METRIC_COUNTER, "# worker core_read events"     )\
    ACTION( worker_event_write,     METRIC_COUNTER, "# worker core_write events"    )\
    ACTION( worker_event_error,     METRIC_COUNTER, "# worker core_error events"    )

rstatus_t core_worker_setup(void);
void core_worker_teardown(void);
void *core_worker_evloop(void *arg);

#endif /* _BB_CORE_WORKER_H_ */
