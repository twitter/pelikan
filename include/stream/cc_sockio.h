/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#pragma once

/**
 * Stream, short for data stream, defines the data IO interface.
 * There are two essential parts for stream: 1) channels that supports stream-
 * oriented transport, such as TCP, UDS, pipe; 2) data structures that serve as
 * the source and/or destination of such data IO, such as memory buffers.
 *
 * Since a stream depends on both channel and buffer types, it is neither easy
 * nor useful to exhaust all combinations in this interface. Instead, this file
 * focuses on the helper functions that ties those two components together.
 *
 * The most common IO pattern is reading into a contiguous and writing from a
 * vector of buffers.
 * Delimiter-based IO may be useful, but often it's sufficient to start with
 * size-based semantics.
 *
 * Because a stream has all the information needed for data IO and followup
 * actions, it is likely the only data structure to pass into an async event-
 * driven framework.
 */


/* NOTE(kyang): until we figure out a common conn interface, cc_sockio and buf_sock
   will be for TCP connections only */

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_stream.h>

#include <cc_define.h>
#include <cc_metric.h>

#include <inttypes.h>
#include <stdlib.h>

#define BUFSOCK_POOLSIZE 0 /* unlimited */

/*          name                type                default             description */
#define SOCKIO_OPTION(ACTION)                                                             \
    ACTION( buf_sock_poolsize,  OPTION_TYPE_UINT,   BUFSOCK_POOLSIZE,   "buf_sock limit" )

typedef struct {
    SOCKIO_OPTION(OPTION_DECLARE)
} sockio_options_st;

/*          name                type            description */
#define SOCKIO_METRIC(ACTION)                                                   \
    ACTION( buf_sock_create,    METRIC_COUNTER, "# buf sock created"           )\
    ACTION( buf_sock_create_ex, METRIC_COUNTER, "# buf sock create exceptions" )\
    ACTION( buf_sock_destroy,   METRIC_COUNTER, "# buf sock destroyed"         )\
    ACTION( buf_sock_curr,      METRIC_GAUGE,   "# buf sock allocated"         )\
    ACTION( buf_sock_borrow,    METRIC_COUNTER, "# buf sock borrowed"          )\
    ACTION( buf_sock_borrow_ex, METRIC_COUNTER, "# buf sock borrow exceptions" )\
    ACTION( buf_sock_return,    METRIC_COUNTER, "# buf sock returned"          )\
    ACTION( buf_sock_active,    METRIC_GAUGE,   "# buf sock being borrowed"    )

typedef struct {
    SOCKIO_METRIC(METRIC_DECLARE)
} sockio_metrics_st;

struct buf_sock {
    /* these fields are useful for resource managmenet */
    STAILQ_ENTRY(buf_sock)  next;
    void                    *owner;
    bool                    free;

    uint64_t                flag;   /* generic flag field to be used by app */
    void                    *data;  /* generic data field to be used by app */
    channel_handler_st       *hdl;   /* use can specify per-channel action */

    struct tcp_conn         *ch;
    struct buf              *rbuf;
    struct buf              *wbuf;
};

STAILQ_HEAD(buf_sock_sqh, buf_sock); /* corresponding header type for the STAILQ */

void sockio_setup(sockio_options_st *options, sockio_metrics_st *metrics);
void sockio_teardown(void);

struct buf_sock *buf_sock_create(void);     /* stream_get_fn */
void buf_sock_destroy(struct buf_sock **);  /* stream_put_fn */

struct buf_sock *buf_sock_borrow(void);     /* stream_get_fn */
void buf_sock_return(struct buf_sock **);   /* stream_put_fn */

void buf_sock_reset(struct buf_sock *);

rstatus_i buf_tcp_read(struct buf_sock *);
rstatus_i buf_tcp_write(struct buf_sock *);

rstatus_i dbuf_tcp_read(struct buf_sock *); /* buf_tcp_read with
                                               doubling buffer */

#ifdef __cplusplus
}
#endif
