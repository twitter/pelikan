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

//! `rustcore` is the core event loop of a server. It is analogous to the
//! `core` library within pelikan and carries out the same function.
//!
//! By default, a pelikan server runs on 3 different threads.
//! - The admin thread is responsible for non-cache related tasks such
//!   as exposing metrics.
//! - The listener thread is responsible for initializing new connections
//!   that are sent to the server.
//! - The worker thread is responsible for reading from connections, performing
//!   cache operations, and sending the results back to the client.
//!
//! The `Core` config takes care of starting these threads once configured
//! with a set of futures to run for each.
//!
//! # How to use this library
//! There are two different ways to use this library.
//!
//! ## With a simple protocol
//! If the serialization/deserialization logic for the desired protocol is
//! simple enough that it can reasonably implement [`Protocol`][0] then
//! you can implement [`Worker`][1]. Once you've implemented `Worker`, then
//! you can use [`worker::default_worker`][2] as the worker task. From there
//! setting up the server is fairly straightforward.
//!
//! ## With a more complicated protocol
//! If the protocol serialization/deserialization doesn't fit with the
//! [`Protocol`][0] trait. Then you'll have to implement the stream
//! reading loop yourself. There are utility methods under the [`util`][3]
//! module that take care of some of the more tricky error handling
//! around reading and writing buffers.
//!
//! For an example of how this is done, see the implementation of
//! [`worker::default_worker`][2].
//!
//! # Example
//! See `pingserver-rs` for a fully-working example of a server with pelikan.
//!
//! [0]: pelikan::protocol::Protocol
//! [1]: crate::Worker
//! [2]: crate::worker::default_worker
//! [3]: crate::util

#[macro_use]
extern crate log;
#[macro_use]
extern crate thiserror;
#[macro_use]
extern crate ccommon;
#[macro_use]
extern crate pin_project;

mod listener;
mod signal;
mod spawn;
mod traits;

pub mod admin;
pub mod errors;
pub mod util;
pub mod worker;

use std::future::Future;
use std::io::Result as IOResult;
use std::pin::Pin;
use std::rc::Rc;

use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc::Receiver;

pub use crate::admin::AdminOptions;
pub use crate::listener::*;
pub use crate::spawn::spawn_local;
pub use crate::traits::{Action, AdminHandler, ClosableStream, Worker};

use crate::spawn::ThreadPinnedFuture;
use crate::traits::WorkerFn;
use crate::worker::{default_worker, WorkerMetrics};

/// Given an AdminHandler and a DataProcessor start up a server
pub fn core_run_tcp<W, H>(
    admin_opts: &AdminOptions,
    server_opts: &ListenerOptions,
    metrics: &'static CoreMetrics,
    admin_handler: H,
    worker: W,
) -> IOResult<()>
where
    W: Worker + Unpin + 'static,
    H: AdminHandler + Send + 'static,
{
    use tokio::sync::mpsc::channel;

    let admin_addr = admin_opts.addr().expect("Invalid socket address");
    let server_addr = server_opts.addr().expect("Invalid socket address");

    let dlog_intvl = admin_opts.dlog_intvl();

    let (send, recv) = channel(1024);

    let mut core = Core::new(move || {
        crate::admin::admin_tcp(admin_addr, admin_handler, dlog_intvl, &metrics.admin)
    })?;
    core.listener(async move {
        crate::tcp_listener(server_addr, send, &metrics.acceptor)
            .await
            .unwrap()
    });
    core.worker(recv, Rc::new(worker), &metrics.worker, default_worker);

    core.run()
}

pub struct Core<A> {
    workers: Runtime,
    admin: A,
    listeners: Vec<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
}

impl<A, F> Core<A>
where
    A: (FnOnce() -> F) + Send + 'static,
    F: Future<Output = IOResult<()>>,
{
    pub fn new(admin: A) -> IOResult<Self> {
        let runtime = Builder::new()
            .enable_io()
            .enable_time()
            .basic_scheduler()
            .build()?;

        Ok(Self {
            workers: runtime,
            admin,
            listeners: vec![],
        })
    }

    /// Register a new listener. These should accept new connections
    /// and then hand them off to a worker.
    ///
    /// All listeners will run on a separate thread from the workers.
    /// This is to mitigate DDOSes and also to prevent the creation of new
    /// connections from affecting the latency of the main thread.
    pub fn listener<L>(&mut self, listener: L) -> &mut Self
    where
        L: Future<Output = ()> + Send + 'static,
    {
        self.listeners.push(Box::pin(listener));
        self
    }

    /// Register a new worker. These are responsible for taking new connections
    /// from the listener and for processing all data sent through those
    /// connections. These will run the core cache logic within pelikan.
    pub fn worker<S, W, Fun>(
        &mut self,
        channel: Receiver<S>,
        state: Rc<W>,
        metrics: &'static WorkerMetrics,
        worker: Fun,
    ) -> &mut Self
    where
        Fun: (for<'a> WorkerFn<'a, W, S>) + 'static,
        W: 'static,
        S: ClosableStream + 'static,
    {
        self.workers
            .spawn(ThreadPinnedFuture::new(crate::worker::worker(
                channel, state, metrics, worker,
            )));
        self
    }

    /// Run the admin handler, the listeners, and the workers all on separate
    /// threads. This will block until they all shut down.
    pub fn run(self) -> IOResult<()> {
        use std::thread;

        let Self {
            admin,
            mut workers,
            listeners,
        } = self;

        let admin_thread = thread::spawn(move || -> IOResult<()> {
            let mut runtime = Builder::new()
                .enable_io()
                .enable_time()
                .basic_scheduler()
                .build()?;

            runtime.block_on(admin())
        });

        let listener_thread = thread::spawn(move || -> IOResult<()> {
            let mut runtime = Builder::new()
                .enable_io()
                .enable_time()
                .basic_scheduler()
                .build()?;

            for listener in listeners {
                runtime.spawn(listener);
            }

            runtime.block_on(crate::signal::wait_for_ctrl_c());

            Ok(())
        });

        workers.block_on(crate::signal::wait_for_ctrl_c());

        admin_thread.join().expect("Admin thread panicked")?;
        listener_thread.join().expect("Listener thread panicked")?;

        Ok(())
    }
}

#[derive(Metrics)]
#[repr(C)]
pub struct CoreMetrics {
    pub worker: WorkerMetrics,
    pub admin: crate::admin::AdminMetrics,
    pub acceptor: TcpListenerMetrics,
}
