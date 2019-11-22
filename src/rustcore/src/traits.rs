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

use pelikan::protocol::{Protocol, StatefulProtocol};

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
    type Protocol: for<'de> Protocol<'de>;

    /// Per-connection state. This is not reinitialized
    /// for each request.
    type State: Default;

    /// Handle a single request and initialize a response.
    fn process_request<'de>(
        &self,
        req: <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as StatefulProtocol>::ResponseState,
        state: &mut Self::State,
    ) -> Action<'de, Self::Protocol>;
}

/// Handler for dealing with requests on the admin port.
pub trait AdminHandler {
    type Protocol: for<'de> Protocol<'de>;

    #[must_use]
    fn process_request<'de>(
        &mut self,
        req: <Self::Protocol as Protocol<'de>>::Request,
        rsp: &mut <Self::Protocol as StatefulProtocol>::ResponseState,
    ) -> Action<'de, Self::Protocol>;
}

/// An action that the admin thread can do after
/// processing a request.
///
/// By default a worker should just send the response.
///
/// # Note
/// This enum should not be matched exhaustively. Adding
/// new variants is not considered a backwards-incompatible
/// change.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Action<'de, P: Protocol<'de>> {
    // Nothing special - sends the response as normal
    Respond(P::Response),
    // Close the connection
    Close,
    // Don't send a response
    NoResponse,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
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
