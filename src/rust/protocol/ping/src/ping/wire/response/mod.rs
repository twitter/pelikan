// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the serialization of `Ping` protocol responses into the wire
//! representation.

use crate::Compose;
use session::Session;
use std::io::Write;

/// A collection of all possible `Ping` responses
pub enum PingResponse {
    Pong,
}

// TODO(bmartin): consider a different trait bound here when reworking buffers.
// We ignore the unused result warnings here because we know we're using a
// buffer with infallible writes (growable buffer). This is *not* guaranteed by
// the current trait bound.
#[allow(unused_must_use)]
impl Compose for PingResponse {
    fn compose(self, dst: &mut Session) {
        match self {
            Self::Pong => {
                dst.write_all(b"PONG\r\n");
            }
        }
    }
}
