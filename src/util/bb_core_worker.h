#ifndef _BB_CORE_WORKER_H_
#define _BB_CORE_WORKER_H_

#include <cc_define.h>
#include <cc_metric.h>

/*          name                    type            description */
#define CORE_WORKER_METRIC(ACTION)                                                   \
    ACTION( worker_event_total,     METRIC_COUNTER, "# worker events returned"      )\
    ACTION( worker_event_loop,      METRIC_COUNTER, "# worker event loops returned" )\
    ACTION( worker_event_read,      METRIC_COUNTER, "# worker core_read events"     )\
    ACTION( worker_event_write,     METRIC_COUNTER, "# worker core_write events"    )\
    ACTION( worker_event_error,     METRIC_COUNTER, "# worker core_error events"    )

typedef struct {
    CORE_WORKER_METRIC(METRIC_DECLARE)
} worker_metrics_st;

#define WORKER_METRIC_INIT(_metrics) do {                                  \
    *(_metrics) = (worker_metrics_st) { CORE_WORKER_METRIC(METRIC_INIT) }; \
} while(0)

rstatus_t core_worker_setup(worker_metrics_st *metrics);
void core_worker_teardown(void);
void *core_worker_evloop(void *arg);

#endif /* _BB_CORE_WORKER_H_ */
