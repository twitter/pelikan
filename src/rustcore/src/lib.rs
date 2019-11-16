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

#[macro_use]
extern crate log;

mod listener;
mod opts;
mod stats;

pub mod admin;
pub mod worker;

pub mod error {
    pub use crate::opts::AddrParseError;
}

use std::future::Future;
use std::io::Result as IOResult;
use std::thread::JoinHandle;

use tokio::runtime::current_thread::Runtime;

use pelikan::core::admin::AdminHandler;
use pelikan::core::DataProcessor;
use pelikan::protocol::{Protocol, QuitRequest};

pub use crate::listener::tcp_listener;
pub use crate::opts::{AdminOptions, ServerOptions};
pub use crate::stats::WorkerMetrics;
pub use crate::worker::worker;

/// Take a future and run it on a separate thread
/// only until that future completes.
///
/// After the future completes it drops all other
/// futures that were spawned on the runtime.
pub fn run_listener<Fn, Fut, T, E>(acceptor: Fn) -> JoinHandle<Result<T, E>>
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + 'static,
    E: From<std::io::Error> + Send + 'static,
    T: Send + 'static,
{
    std::thread::spawn(move || -> Result<T, E> {
        let mut runtime = Runtime::new()?;

        runtime.block_on(acceptor())
    })
}

/// Given an AdminHandler and a DataProcessor start up a server
pub fn core_run_tcp<P, H>(
    admin_opts: &AdminOptions,
    server_opts: &ServerOptions,
    worker_metrics: &'static WorkerMetrics,
    admin_handler: H,
    data_processor: P,
) -> IOResult<()>
where
    P: DataProcessor + 'static,
    H: AdminHandler + Send + 'static,
    <H::Protocol as Protocol>::Request: QuitRequest,
{
    use tokio::sync::mpsc::channel;

    let admin_addr = admin_opts.addr().expect("Invalid socket address");
    let server_addr = server_opts.addr().expect("Invalid socket address");

    let dlog_intvl = admin_opts.dlog_intvl();
    
    let (send, recv) = channel(1024);
    
    let mut core = Core::new(move || crate::admin::admin_tcp(admin_addr, admin_handler, dlog_intvl))?;
    core
        .listener(async move { 
            crate::tcp_listener(server_addr, send).await.unwrap() 
        })
        .worker(async move { 
            crate::worker(recv, data_processor, &worker_metrics).await.unwrap()
        });

    core.run()
}

pub struct Core<A> {
    workers: Runtime,
    admin: A,
}

impl<A, F> Core<A>
where
    A: (FnOnce() -> F) + Send + 'static,
    F: Future<Output = IOResult<()>> 
{
    pub fn new(admin: A) -> IOResult<Self> {
        Ok(Self {
            workers: Runtime::new()?,
            admin
        })
    }

    pub fn listener<L>(&mut self, listener: L) -> &mut Self
    where
        L: Future<Output = ()> + Send + 'static
    {
        self.workers.spawn(listener);
        self
    }

    pub fn worker<W>(&mut self, worker: W) -> &mut Self 
    where
        W: Future<Output = ()> + 'static
    {
        self.workers.spawn(worker);
        self
    }

    pub fn run(self) -> IOResult<()> {
        use std::thread;

        let Self { admin, mut workers } = self;
        
        let admin_thread = thread::spawn(move || -> IOResult<()> {
            let mut runtime = Runtime::new()?;
            runtime.block_on(admin())
        });
        
        // TODO: Block on a signal handler here
        workers.run()
            .expect("Error while running workers");

        admin_thread.join().expect("Admin thread panicked")?;

        Ok(())
    }
}