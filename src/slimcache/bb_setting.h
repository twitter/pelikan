#ifndef _BB_SETTING_H_
#define _BB_SETTING_H_

#include <cuckoo/bb_cuckoo.h>
#include <memcache/bb_request.h>

#include <cc_array.h>
#include <cc_log.h>
#include <cc_mbuf.h>
#include <cc_metric.h>
#include <cc_option.h>
#include <cc_signal.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>


/* option related */
/*          name            type                default     description */
#define SERVER_OPTION(ACTION)                                                           \
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,       "interfaces listening on"  )\
    ACTION( server_port,    OPTION_TYPE_STR,    "22222",    "port listening on"        )

/* we compose our setting by including options needed by modules we use */
#define SETTING(ACTION)             \
    ARRAY_OPTION(ACTION)            \
    SOCKIO_OPTION(ACTION)           \
    CUCKOO_OPTION(ACTION)           \
    LOG_OPTION(ACTION)              \
    MBUF_OPTION(ACTION)             \
    TCP_OPTION(ACTION)              \
    REQUEST_OPTION(ACTION)          \
    SERVER_OPTION(ACTION)

struct setting {
    SETTING(OPTION_DECLARE)
};

#endif /* _BB_SETTING_H_ */
