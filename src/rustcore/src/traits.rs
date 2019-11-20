// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use pelikan::protocol::Protocol;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

/// A stream that can be gracefully closed.
///
/// If this is handled properly by the drop impl
/// then this function can be a no-op.
pub trait ClosableStream {
    fn close(&self) -> std::io::Result<()>;
}

/// A type that accepts a certain type of request and
/// initializes a response.
pub trait Worker {
    /// The protocol for the wire format.
    type Protocol: Protocol;

    /// Per-connection state. This is not reinitialized
    /// for each request.
    type State: Default;

    /// Handle a single request and initialize a response.
    fn process_request(
        &self,
        req: &mut <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as Protocol>::Response,
        state: &mut Self::State,
    ) -> WorkerAction;
}

/// An action that the worker thread can do after
/// processing a request.
///
/// By default a worker should just send the response.
///
/// # Note
/// This enum should not be matched exhaustively. Adding
/// new variants is not considered a backwards-incompatible
/// change.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum WorkerAction {
    // Nothing special - sends the response as normal
    None,
    // Close the connection
    Close,
    // Don't send a response
    NoResponse,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

impl Default for WorkerAction {
    fn default() -> Self {
        Self::None
    }
}

#[cfg(unix)]
impl ClosableStream for tokio::net::UnixStream {
    fn close(&self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }
}

impl ClosableStream for tokio::net::TcpStream {
    fn close(&self) -> std::io::Result<()> {
        self.shutdown(std::net::Shutdown::Both)
    }
}
