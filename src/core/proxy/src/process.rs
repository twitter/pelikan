// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::proxy::ListenerConfig;
use config::proxy::FrontendConfig;
use config::proxy::BackendConfig;
use crate::*;
use std::thread::JoinHandle;

pub struct ProcessBuilder<BackendParser, BackendRequest, BackendResponse, FrontendParser, FrontendRequest, FrontendResponse> {
    admin: AdminBuilder,
    backend: BackendBuilder<BackendParser, BackendRequest, BackendResponse>,
    frontend: FrontendBuilder<FrontendParser, FrontendRequest, FrontendResponse, BackendRequest, BackendResponse>,
    listener: ListenerBuilder,
    log_drain: Box<dyn Drain>,
    // workers: WorkersBuilder<Parser, Request, Response, Storage>,
}

impl<BackendParser, BackendRequest, BackendResponse, FrontendParser, FrontendRequest, FrontendResponse> ProcessBuilder<BackendParser, BackendRequest, BackendResponse, FrontendParser, FrontendRequest, FrontendResponse>
where
    BackendParser: 'static + Parse<BackendResponse> + Clone + Send,
    BackendRequest: 'static + Send + Compose + From<FrontendRequest> + Compose,
    BackendResponse: 'static + Compose + Send,
    FrontendParser: 'static + Parse<FrontendRequest> + Clone + Send,
    FrontendRequest: 'static + Send,
    FrontendResponse: 'static + Compose + Send,
    FrontendResponse: From<BackendResponse> + Compose,
{   
    pub fn new<T: AdminConfig + FrontendConfig + BackendConfig + TlsConfig + ListenerConfig>(
        config: &T,
        log_drain: Box<dyn Drain>,
        backend_parser: BackendParser,
        frontend_parser: FrontendParser,
    ) -> Result<Self> {
        let admin = AdminBuilder::new(config)?;
        let backend = BackendBuilder::new(config, backend_parser, 1)?;
        let frontend = FrontendBuilder::new(config, frontend_parser, 1)?;
        let listener = ListenerBuilder::new(config)?;
        // let workers = WorkersBuilder::new(config, parser, storage)?;

        Ok(Self {
            admin,
            backend,
            frontend,
            listener,
            log_drain,
        })
    }

    pub fn version(mut self, version: &str) -> Self {
        self.admin.version(version);
        self
    }

    pub fn spawn(self) -> Process {
        let mut thread_wakers = vec![self.listener.waker()];
        thread_wakers.extend_from_slice(&self.backend.wakers());
        thread_wakers.extend_from_slice(&self.frontend.wakers());

        // channel for the parent `Process` to send `Signal`s to the admin thread
        let (signal_tx, signal_rx) = bounded(QUEUE_CAPACITY);

        // queues for the `Admin` to send `Signal`s to all sibling threads
        let (mut signal_queue_tx, mut signal_queue_rx) =
            Queues::new(vec![self.admin.waker()], thread_wakers, QUEUE_CAPACITY);

        // queues for the `Listener` to send `Session`s to the worker threads
        let (mut listener_session_queues, worker_session_queues) = Queues::new(
            vec![self.listener.waker()],
            self.frontend.wakers(),
            QUEUE_CAPACITY,
        );

        let (fe_data_queues, be_data_queues) = Queues::new(
            self.frontend.wakers(),
            self.backend.wakers(),
            QUEUE_CAPACITY,
        );

        let mut admin = self
            .admin
            .build(self.log_drain, signal_rx, signal_queue_tx.remove(0));

        let mut listener = self
            .listener
            .build(signal_queue_rx.remove(0), listener_session_queues.remove(0));

        let be_threads = be_data_queues.len();

        let mut backend_workers = self.backend.build(be_data_queues, signal_queue_rx.drain(0..be_threads).collect());
        let mut frontend_workers = self.frontend.build(fe_data_queues, worker_session_queues, signal_queue_rx);

        let admin = std::thread::Builder::new()
            .name(format!("{}_admin", THREAD_PREFIX))
            .spawn(move || admin.run())
            .unwrap();

        let listener = std::thread::Builder::new()
            .name(format!("{}_listener", THREAD_PREFIX))
            .spawn(move || listener.run())
            .unwrap();

        let backend = backend_workers.drain(..).enumerate().map(|(i, mut b)| {
            std::thread::Builder::new()
            .name(format!("{}_be_{}", THREAD_PREFIX, i))
            .spawn(move || b.run())
            .unwrap()
        }).collect();

        let frontend = frontend_workers.drain(..).enumerate().map(|(i, mut f)| {
            std::thread::Builder::new()
            .name(format!("{}_fe_{}", THREAD_PREFIX, i))
            .spawn(move || f.run())
            .unwrap()
        }).collect();

        Process {
            admin,
            backend,
            frontend,
            listener,
            signal_tx,
        }
    }
}

pub struct Process {
    admin: JoinHandle<()>,
    backend: Vec<JoinHandle<()>>,
    frontend: Vec<JoinHandle<()>>,
    listener: JoinHandle<()>,
    signal_tx: Sender<Signal>,
}

impl Process {
    /// Attempts to gracefully shutdown the `Process` by sending a shutdown to
    /// each thread and then waiting to join those threads.
    ///
    /// Will terminate ungracefully if it encounters an error in sending a
    /// shutdown to any of the threads.
    ///
    /// This function will block until all threads have terminated.
    pub fn shutdown(self) {
        // this sends a shutdown to the admin thread, which will broadcast the
        // signal to all sibling threads in the process
        if self.signal_tx.try_send(Signal::Shutdown).is_err() {
            fatal!("error sending shutdown signal to thread");
        }

        // wait and join all threads
        self.wait()
    }

    /// Will block until all threads terminate. This should be used to keep the
    /// process alive while the child threads run.
    pub fn wait(self) {
        for thread in self.frontend {
            let _ = thread.join();
        }
        for thread in self.backend {
            let _ = thread.join();
        }
        let _ = self.listener.join();
        let _ = self.admin.join();
    }
}
