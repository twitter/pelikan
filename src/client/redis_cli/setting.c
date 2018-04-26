#include "setting.h"

struct setting setting = {
    { REDISCLI_OPTION(OPTION_INIT)  },
    { REQUEST_OPTION(OPTION_INIT)   },
    { RESPONSE_OPTION(OPTION_INIT)  },
    { BUF_OPTION(OPTION_INIT)       },
    { DBUF_OPTION(OPTION_INIT)      },
    { DEBUG_OPTION(OPTION_INIT)     },
    { SOCKIO_OPTION(OPTION_INIT)    },
    { TCP_OPTION(OPTION_INIT)       },
};

unsigned int nopt = OPTION_CARDINALITY(struct setting);
