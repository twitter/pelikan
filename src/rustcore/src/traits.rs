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

use std::future::Future;
use std::rc::Rc;

use ccommon::buf::OwnedBuf;
use pelikan::protocol::Protocol;

use crate::WorkerMetrics;

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Empty {}

/// A stream that can be gracefully closed.
///
/// If this is handled properly by the drop impl
/// then this function can be a no-op.
pub trait ClosableStream {
    fn close(&self) -> std::io::Result<()>;
}

/// Worker trait that mirrors how protocols are handled in
/// pelikan core.
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
    ) -> Action;
}

/// Handler for dealing with requests on the admin port.
pub trait AdminHandler {
    type Protocol: Protocol;

    #[must_use]
    fn process_request<'de>(
        &mut self,
        req: &mut <Self::Protocol as Protocol>::Request,
        rsp: &mut <Self::Protocol as Protocol>::Response,
    ) -> Action;
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
pub enum Action {
    // Nothing special - sends the response as normal
    Respond,
    // Close the connection
    Close,
    // Don't send a response
    NoResponse,

    #[doc(hidden)]
    __Nonexhaustive(Empty),
}

/// Internal trait type used mainly to abstract over a function type
pub trait WorkerFn<'a, W: 'static, S: 'a> {
    type Fut: Future<Output = ()> + 'a;

    fn eval(
        &self,
        state: Rc<W>,
        stream: &'a mut S,
        rbuf: &'a mut OwnedBuf,
        wbuf: &'a mut OwnedBuf,
        metrics: &'static WorkerMetrics,
    ) -> Self::Fut;
}

impl<'a, W, S, Fut, Fun> WorkerFn<'a, W, S> for Fun
where
    Fun: Fn(Rc<W>, &'a mut S, &'a mut OwnedBuf, &'a mut OwnedBuf, &'static WorkerMetrics) -> Fut,
    Fut: Future<Output = ()> + 'a,
    W: 'static,
    S: 'a,
{
    type Fut = Fut;

    fn eval(
        &self,
        state: Rc<W>,
        stream: &'a mut S,
        rbuf: &'a mut OwnedBuf,
        wbuf: &'a mut OwnedBuf,
        metrics: &'static WorkerMetrics,
    ) -> Fut {
        self(state, stream, rbuf, wbuf, metrics)
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
