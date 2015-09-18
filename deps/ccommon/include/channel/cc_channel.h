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
 * Note(yao): a channel is an abstration of stream-oriented transport. In
 * practice, it can be implemented over a number of media: TCP/UDP, Linux domain
 * sockets, files, pipes, shared memory, etc. While we may start with only one
 * or two of these potential underlying I/O mechanisms, it is beneficial to
 * build an abstraction to provide a unified interface on top of them.
 *
 * From a service's perspective it needs a few essential guarantees: 1) over the
 * same channel requests/data arrive in order; 2) user should be able to specify
 * and control how much data is being read/written; 3) read/write of the same
 * data should happen at most once, e.g. no duplicates, but it is possible to
 * fail.
 *
 * The idea described here is not dissimilar to the use of channels in Plan 9 or
 * nanomsg.
 */

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_queue.h>
#include <cc_signal.h>

#include <stdbool.h>
#include <sys/types.h>

/**
 * There needs to be two levels of channels: meta and peer. "Meta" channels are
 * called such because their purpose is to manage channels. They are always on
 * by default and listening. They usually don't process data but instead spin
 * off new channels that are connected to their peers to transport data.
 *
 * Among peer channels, it is worth noting that difference exists between the
 * ones that are reactive (server) from the proactive (client) ones. One key
 * difference is server channels are never opened proactively, and hence a
 * server implementation does not need to provide a channel open handler, instead,
 * a meta channel implementation needs to provide ways to accept a new channel
 * request from a client.
 */

/* channel related data types */
typedef void * channel_t;
typedef void * address_t;
typedef int ch_id_t;

typedef enum channel_level {
    CHANNEL_INVALID,
    CHANNEL_META,
    CHANNEL_BASE
} ch_level_t;

/**
 * To give implementation a chance to explicitly manage memory, we separate out
 * the interfaces for acquiring and releasing a channel as a resource, from
 * those that prepare the channels for use/reuse.
 */
typedef channel_t (* channel_get_fn)(void);
typedef void (* channel_put_fn)(channel_t *);

/**
 * Operations on the channel, note that no memory allocation/deallocation should
 * happen within the functions below. Delegate resource management to the above
 * callbacks.
 */
typedef bool (* channel_accept_fn)(channel_t, channel_t);
typedef void (* channel_reject_fn)(channel_t);

typedef bool (* channel_open_fn)(address_t, channel_t);
typedef void (* channel_term_fn)(channel_t);
typedef ssize_t (* channel_recv_fn)(channel_t, void *, size_t);
typedef ssize_t (* channel_send_fn)(channel_t, void *, size_t);
typedef ch_id_t (* channel_id_fn)(channel_t);

typedef struct channel_handler {
    channel_accept_fn   accept;
    channel_reject_fn   reject;

    channel_open_fn     open;
    channel_term_fn     term;
    channel_recv_fn     recv;
    channel_send_fn     send;
    channel_id_fn       rid;
    channel_id_fn       wid;
} channel_handler_t;

/* channel states, this is to be revised later (yao) */
enum {
    CHANNEL_UNKNOWN = 0,
    CHANNEL_LISTEN,         /* listening */
    CHANNEL_OPEN,           /* opening */
    CHANNEL_ESTABLISHED,
    CHANNEL_TERM,           /* to be closed, don't need a closing state yet */
    CHANNEL_ERROR,          /* unrecoverable error occurred */

    CHANNEL_SENTINEL
};

static inline int
channel_sigpipe_ignore(void)
{
    /* if sigpipe is not ignored, the application will crash/exit upon signal */
    return signal_override(SIGPIPE, "ignoring sigpipe", 0, 0, SIG_IGN);
}
#ifdef __cplusplus
}
#endif
