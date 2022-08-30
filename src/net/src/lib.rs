// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod connector;
mod listener;
mod stream;
mod tcp;
mod tls_tcp;

pub use connector::*;
pub use listener::*;
pub use stream::*;
pub use tcp::*;
pub use tls_tcp::*;

pub mod event {
    pub use mio::event::*;
}

pub use mio::*;

use core::fmt::Debug;
use core::ops::Deref;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use foreign_types_shared::{ForeignType, ForeignTypeRef};
use rustcommon_metrics::*;

type Result<T> = std::io::Result<T>;

// stats

counter!(
    TCP_ACCEPT,
    "number of TCP streams passively opened with accept"
);
counter!(
    TCP_CONNECT,
    "number of TCP streams actively opened with connect"
);
counter!(TCP_CLOSE, "number of TCP streams closed");
gauge!(TCP_CONN_CURR, "current number of open TCP streams");
counter!(TCP_RECV_BYTE, "number of bytes received on TCP streams");
counter!(TCP_SEND_BYTE, "number of bytes sent on TCP streams");

counter!(STREAM_ACCEPT, "number of calls to accept");
counter!(
    STREAM_ACCEPT_EX,
    "number of times calling accept resulted in an exception"
);
counter!(STREAM_SHUTDOWN, "number of streams gracefully shutdown");
counter!(
    STREAM_SHUTDOWN_EX,
    "number of exceptions while attempting to gracefully shutdown a stream"
);
counter!(
    STREAM_HANDSHAKE,
    "number of times stream handshaking was attempted"
);
counter!(
    STREAM_HANDSHAKE_EX,
    "number of exceptions while handshaking"
);
