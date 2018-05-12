#pragma once

#include "core/data/server.h"

#include "protocol/data/redis_include.h"

#include <buffer/cc_buf.h>
#include <buffer/cc_dbuf.h>
#include <cc_debug.h>
#include <cc_log.h>
#include <cc_option.h>
#include <channel/cc_tcp.h>
#include <stream/cc_sockio.h>

/*          name            type                default description */
#define RESPCLI_OPTION(ACTION)                                                            \
    ACTION( server_host,    OPTION_TYPE_STR,    NULL,           "server, NULL is loopback" )\
    ACTION( data_port,      OPTION_TYPE_STR,    SERVER_PORT,    "data plane server port"   )

typedef struct {
    RESPCLI_OPTION(OPTION_DECLARE)
} respcli_options_st;

struct setting {
    /* top-level */
    respcli_options_st      respcli;
    /* application modules */
    request_options_st      request;
    response_options_st     response;
    /* ccommon libraries */
    buf_options_st          buf;
    dbuf_options_st         dbuf;
    debug_options_st        debug;
    sockio_options_st       sockio;
    tcp_options_st          tcp;
};

extern struct setting setting;
extern unsigned int nopt;
