/*
 * ccommon - a cache common library.
 * Copyright (C) 2015 Twitter, Inc.
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

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_debug.h>
#include <cc_metric.h>
#include <channel/cc_channel.h>

#include <stdbool.h>
#include <unistd.h>

/**
 * This implements the channel interface for pipes
 */

#define PIPE_POOLSIZE 1         /* Currently our applications only use 1 pipe conn */

/*          name                 type            description */
#define PIPE_METRIC(ACTION) \
    ACTION( pipe_conn_create,    METRIC_COUNTER, "# pipe connections created"    )\
    ACTION( pipe_conn_create_ex, METRIC_COUNTER, "# pipe conn create exceptions" )\
    ACTION( pipe_conn_destroy,   METRIC_COUNTER, "# pipe connections destroyed"  )\
    ACTION( pipe_conn_curr ,     METRIC_GAUGE,   "# pipe conn allocated"         )\
    ACTION( pipe_conn_borrow,    METRIC_COUNTER, "# pipe connections borrowed"   )\
    ACTION( pipe_conn_borrow_ex, METRIC_COUNTER, "# pipe conn borrow exceptions" )\
    ACTION( pipe_conn_return,    METRIC_COUNTER, "# pipe connections returned"   )\
    ACTION( pipe_conn_active,    METRIC_GAUGE,   "# pipe conn being borrowed"    )\
    ACTION( pipe_open,           METRIC_COUNTER, "# pipe connects made"          )\
    ACTION( pipe_open_ex,        METRIC_COUNTER, "# pipe connect exceptions"     )\
    ACTION( pipe_close,          METRIC_COUNTER, "# pipe connection closed"      )\
    ACTION( pipe_recv,           METRIC_COUNTER, "# recv attempted"              )\
    ACTION( pipe_recv_ex,        METRIC_COUNTER, "# recv exceptions"             )\
    ACTION( pipe_recv_byte,      METRIC_COUNTER, "# bytes received"              )\
    ACTION( pipe_send,           METRIC_COUNTER, "# send attempted"              )\
    ACTION( pipe_send_ex,        METRIC_COUNTER, "# send exceptions"             )\
    ACTION( pipe_send_byte,      METRIC_COUNTER, "# bytes sent"                  )\
    ACTION( pipe_flag_ex,        METRIC_COUNTER, "# pipe flag exceptions"        )

typedef struct {
    PIPE_METRIC(METRIC_DECLARE)
} pipe_metrics_st;

#define PIPE_METRIC_INIT(_metrics) do {                           \
    *(_metrics) = (pipe_metrics_st) { PIPE_METRIC(METRIC_INIT) }; \
} while (0)

/*          name                type                default         description */
#define PIPE_OPTION(ACTION) \
    ACTION( pipe_poolsize,      OPTION_TYPE_UINT,   PIPE_POOLSIZE,  "pipe conn pool size" )

struct pipe_conn {
    STAILQ_ENTRY(pipe_conn) next;       /* for pool */
    bool                    free;       /* in use? */

    int                     fd[2];      /* file descriptors */

    size_t                  recv_nbyte; /* # bytes read */
    size_t                  send_nbyte; /* # bytes written */

    unsigned                state:4;    /* defined as above */
    unsigned                flags;      /* annotation fields */

    err_t                   err;        /* errno */
};

STAILQ_HEAD(pipe_conn_sqh, pipe_conn); /* corresponding header type for the STAILQ */

void pipe_setup(pipe_metrics_st *metrics);
void pipe_teardown(void);

/* functions for managing pipe connection structs */

/* creation/destruction */
struct pipe_conn *pipe_conn_create(void);
void pipe_conn_destroy(struct pipe_conn **c);

/* initialize a pipe_conn struct for use */
void pipe_conn_reset(struct pipe_conn *c);

/* pool functions */
void pipe_conn_pool_create(uint32_t max);
void pipe_conn_pool_destroy(void);
struct pipe_conn *pipe_conn_borrow(void);
void pipe_conn_return(struct pipe_conn **c);

/* functions for using pipe connections */

/* open/close pipe */
bool pipe_open(void *addr, struct pipe_conn *c);
void pipe_close(struct pipe_conn *c);

/* send/recv on pipe */
ssize_t pipe_recv(struct pipe_conn *c, void *buf, size_t nbyte);
ssize_t pipe_send(struct pipe_conn *c, void *buf, size_t nbyte);

static inline ch_id_t pipe_read_id(struct pipe_conn *c)
{
    return c->fd[0];
}

static inline ch_id_t pipe_write_id(struct pipe_conn *c)
{
    return c->fd[1];
}

/* set pipe flags */
void pipe_set_blocking(struct pipe_conn *c);
void pipe_set_nonblocking(struct pipe_conn *c);

#ifdef __cplusplus
}
#endif
