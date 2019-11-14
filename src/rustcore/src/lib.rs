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

mod acceptors;
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

use pelikan::core::admin::AdminHandler;
use pelikan::core::DataProcessor;
use pelikan::protocol::{Protocol, QuitRequest};

pub use crate::acceptors::tcp_acceptor;
pub use crate::opts::{AdminOptions, ServerOptions};
pub use crate::stats::WorkerMetrics;
pub use crate::worker::worker;

/// Take a future and run it on a separate thread
/// only until that future completes.
///
/// After the future completes it drops all other
/// futures that were spawned on the runtime.
pub fn run_acceptor<Fn, Fut, T, E>(acceptor: Fn) -> JoinHandle<Result<T, E>>
where
    Fn: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, E>> + 'static,
    E: From<std::io::Error> + Send + 'static,
    T: Send + 'static,
{
    use tokio::runtime::current_thread::Runtime;

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
    use tokio::runtime::current_thread::Runtime;
    use tokio::sync::mpsc::channel;

    let admin_addr = admin_opts.addr().expect("Invalid socket address");
    let server_addr = server_opts.addr().expect("Invalid socket address");

    let dlog_intvl = admin_opts.dlog_intvl();

    let admin_thread =
        run_acceptor(move || crate::admin::admin_tcp(admin_addr, admin_handler, dlog_intvl));

    let (send, recv) = channel(1024);

    let acceptor_thread = run_acceptor(move || crate::tcp_acceptor(server_addr, send));

    let mut runtime = Runtime::new()?;
    let res = runtime.block_on(crate::worker(recv, data_processor, &worker_metrics));

    let acceptor_res = acceptor_thread.join();
    let admin_res = admin_thread.join();

    acceptor_res.expect("Acceptor panicked")?;
    admin_res.expect("Admin thread panicked")?;

    res
}
