#include "stats.h"

struct stats stats = {
    { PROCINFO_METRIC(METRIC_INIT)      },
    { PROCESS_METRIC(METRIC_INIT)       },
    { PARSE_REQ_METRIC(METRIC_INIT)     },
    { COMPOSE_RSP_METRIC(METRIC_INIT)   },
    { KLOG_METRIC(METRIC_INIT)          },
    { REQUEST_METRIC(METRIC_INIT)       },
    { RESPONSE_METRIC(METRIC_INIT)      },
    { SEG_METRIC(METRIC_INIT)          },
    { CORE_SERVER_METRIC(METRIC_INIT)   },
    { CORE_WORKER_METRIC(METRIC_INIT)   },
    { BUF_METRIC(METRIC_INIT)           },
    { DBUF_METRIC(METRIC_INIT)          },
    { EVENT_METRIC(METRIC_INIT)         },
    { LOG_METRIC(METRIC_INIT)           },
    { SOCKIO_METRIC(METRIC_INIT)        },
    { TCP_METRIC(METRIC_INIT)           },
    { TIMING_WHEEL_METRIC(METRIC_INIT)  },
};

unsigned int nmetric = METRIC_CARDINALITY(stats);
