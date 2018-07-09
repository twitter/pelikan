#include "setting.h"

struct setting setting = {
    { CDB_OPTION(OPTION_INIT)       },
    { ADMIN_OPTION(OPTION_INIT)     },
    { SERVER_OPTION(OPTION_INIT)    },
    { WORKER_OPTION(OPTION_INIT)    },
    { PROCESS_OPTION(OPTION_INIT)   },
    { KLOG_OPTION(OPTION_INIT)      },
    { REQUEST_OPTION(OPTION_INIT)   },
    { RESPONSE_OPTION(OPTION_INIT)  },
    { TIME_OPTION(OPTION_INIT)      },
    { ARRAY_OPTION(OPTION_INIT)     },
    { BUF_OPTION(OPTION_INIT)       },
    { DBUF_OPTION(OPTION_INIT)      },
    { DEBUG_OPTION(OPTION_INIT)     },
    { SOCKIO_OPTION(OPTION_INIT)    },
    { TCP_OPTION(OPTION_INIT)       },
};

unsigned int nopt = OPTION_CARDINALITY(struct setting);
