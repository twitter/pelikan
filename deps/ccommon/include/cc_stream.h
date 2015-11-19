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

#ifdef __cplusplus
extern "C" {
#endif

#include <channel/cc_channel.h>

#include <inttypes.h>

typedef void * iobuf_p; // TODO(yao): move into a generic buffer interface
typedef ssize_t (*io_size_fn)(channel_p, iobuf_p, size_t);
typedef ssize_t (*io_limiter_fn)(channel_p, iobuf_p, const char *);

typedef void * stream_p;

typedef stream_p (* stream_get_fn)(void);
typedef void (* stream_put_fn)(stream_p);

/**
 * an implementation of a stream should look something like the following
 *
struct stream {
    // these fields are useful for resource managmenet
    STAILQ_ENTRY(stream)    next;
    void                    *owner;
    bool                    free;

    channel_p               ch;
    iobuf_p                 rbuf;
    iocb_size_fn            read;
    iobuf_p                 wbuf;
    iocb_size_fn            write;
};
 */

#ifdef __cplusplus
}
#endif
