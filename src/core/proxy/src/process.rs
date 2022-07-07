// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::admin::Admin;
use crate::admin::AdminBuilder;
use crate::backend::BackendWorkerBuilder;
use crate::frontend::FrontendWorkerBuilder;
use crate::listener::ListenerBuilder;
use crate::*;
use common::signal::Signal;
use config::proxy::{BackendConfig, FrontendConfig, ListenerConfig};
use config::AdminConfig;
use config::ServerConfig;
use config::TlsConfig;
use crossbeam_channel::bounded;
use crossbeam_channel::Sender;
use logger::Drain;
use net::Waker;
use protocol_common::*;
use queues::Queues;
use std::sync::Arc;
use std::thread::JoinHandle;

pub const FRONTEND_THREADS: usize = 1;
pub const BACKEND_THREADS: usize = 1;
pub const BACKEND_POOLSIZE: usize = 1;

pub struct ProcessBuilder<RequestParser, Request, ResponseParser, Response> {
    admin: Admin,
    listener: Listener,
    frontends: Vec<FrontendWorker<RequestParser, Request, Response>>,
    backends: Vec<BackendWorker<ResponseParser, Request, Response>>,
    signal_tx: Sender<Signal>,
}

impl<RequestParser, Request, ResponseParser, Response>
    ProcessBuilder<RequestParser, Request, ResponseParser, Response>
where
    RequestParser: 'static + Clone + Send + Parse<Request>,
    Request: 'static + Send + Compose,
    ResponseParser: 'static + Clone + Send + Parse<Response>,
    Response: 'static + Send + Compose,
{
    pub fn new<T: AdminConfig + ListenerConfig + BackendConfig + FrontendConfig + TlsConfig>(
        config: T,
        request_parser: RequestParser,
        response_parser: ResponseParser,
        log_drain: Box<dyn Drain>,
    ) -> Result<Self> {
        // initialize the clock
        common::time::refresh_clock();

        let admin_builder = AdminBuilder::new(&config, log_drain).unwrap_or_else(|e| {
            error!("failed to initialize admin: {}", e);
            std::process::exit(1);
        });
        let admin_waker = admin_builder.waker();

        let listener_builder = ListenerBuilder::new(&config)?;
        let listener_waker = listener_builder.waker();

        let mut frontend_builders = Vec::new();
        for _ in 0..config.frontend().threads() {
            frontend_builders.push(FrontendWorkerBuilder::new(&config, request_parser.clone())?);
        }
        let frontend_wakers: Vec<Arc<Waker>> =
            frontend_builders.iter().map(|v| v.waker()).collect();

        let mut backend_builders = Vec::new();
        for _ in 0..config.backend().threads() {
            backend_builders.push(BackendWorkerBuilder::new(&config, response_parser.clone())?);
        }
        let backend_wakers: Vec<Arc<Waker>> = backend_builders.iter().map(|v| v.waker()).collect();

        let mut thread_wakers = vec![listener_waker.clone()];
        thread_wakers.extend_from_slice(&backend_wakers);
        thread_wakers.extend_from_slice(&frontend_wakers);

        // channel for the parent `Process` to send `Signal`s to the admin thread
        let (signal_tx, signal_rx) = bounded(QUEUE_CAPACITY);

        // queues for the `Admin` to send `Signal`s to all sibling threads
        let (mut signal_queue_tx, mut signal_queue_rx) =
            Queues::new(vec![admin_waker], thread_wakers, QUEUE_CAPACITY);

        let (mut queues_listener_session, mut queues_worker_session) = Queues::new(
            vec![listener_waker],
            frontend_wakers.clone(),
            QUEUE_CAPACITY,
        );
        let (mut queues_frontend_data, mut queues_backend_data) =
            Queues::new(frontend_wakers, backend_wakers, QUEUE_CAPACITY);

        let backends: Vec<BackendWorker<ResponseParser, Request, Response>> = backend_builders
            .drain(..)
            .map(|v| v.build(signal_queue_rx.remove(0), queues_backend_data.remove(0)))
            .collect();

        let frontends: Vec<FrontendWorker<RequestParser, Request, Response>> = frontend_builders
            .drain(..)
            .map(|v| {
                v.build(
                    signal_queue_rx.remove(0),
                    queues_worker_session.remove(0),
                    queues_frontend_data.remove(0),
                )
            })
            .collect();
        let listener = listener_builder.build(queues_listener_session.remove(0));

        let admin = admin_builder.build(signal_queue_tx.remove(0), signal_rx);

        Ok(Self {
            admin,
            listener,
            frontends,
            backends,
            signal_tx,
        })
    }

    #[allow(clippy::vec_init_then_push)]
    pub fn spawn(mut self) -> Process {
        let admin = std::thread::Builder::new()
            .name("pelikan_admin".to_string())
            .spawn(move || self.admin.run())
            .unwrap();

        let listener = std::thread::Builder::new()
            .name("pelikan_listener".to_string())
            .spawn(move || self.listener.run())
            .unwrap();

        let mut frontend = Vec::new();
        for (id, fe) in self.frontends.drain(..).enumerate() {
            frontend.push(
                std::thread::Builder::new()
                    .name(format!("pelikan_fe_{}", id))
                    .spawn(move || fe.run())
                    .unwrap(),
            )
        }

        let mut backend = Vec::new();
        for (id, be) in self.backends.drain(..).enumerate() {
            backend.push(
                std::thread::Builder::new()
                    .name(format!("pelikan_be_{}", id))
                    .spawn(move || be.run())
                    .unwrap(),
            )
        }

        Process {
            admin,
            listener,
            frontend,
            backend,
            signal_tx: self.signal_tx,
        }
    }
}

pub struct Process {
    admin: JoinHandle<()>,
    listener: JoinHandle<()>,
    backend: Vec<JoinHandle<()>>,
    frontend: Vec<JoinHandle<()>>,
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
