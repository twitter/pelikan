#include "setting.h"

struct setting setting = {
    { PINGSERVER_OPTION(OPTION_INIT)},
    { ADMIN_OPTION(OPTION_INIT)     },
    { SERVER_OPTION(OPTION_INIT)    },
    { WORKER_OPTION(OPTION_INIT)    },
    { ARRAY_OPTION(OPTION_INIT)     },
    { BUF_OPTION(OPTION_INIT)       },
    { DBUF_OPTION(OPTION_INIT)      },
    { DEBUG_OPTION(OPTION_INIT)     },
    { SOCKIO_OPTION(OPTION_INIT)    },
    { TCP_OPTION(OPTION_INIT)       },
};

unsigned int nopt = OPTION_CARDINALITY(setting);
