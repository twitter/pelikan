// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![allow(dead_code)]
#![allow(unused_imports)]

#[macro_use]
extern crate logger;

use protocol_common::Compose;
use protocol_common::Parse;
use session_common::ClientSession;
use std::fmt::Debug;
use mpmc::Queue;
use net::event::{Event, Events};
use net::{Interest, TcpListener, TcpStream, Token};
// use poll::Poll;
use slab::Slab;
use std::collections::VecDeque;
use std::io::*;
use std::net::SocketAddr;

mod admin;
mod backend;
// mod event_loop;
mod frontend;
mod listener;
mod poll;
mod process;

pub use admin::PERCENTILES;
use backend::BackendWorker;
// use event_loop::EventLoop;
use frontend::FrontendWorker;
use listener::Listener;
pub use process::{Process, ProcessBuilder};

type Result<T> = std::result::Result<T, std::io::Error>;

use rustcommon_metrics::*;

counter!(TCP_ACCEPT_EX);

// The default buffer size is matched to the upper-bound on TLS fragment size as
// per RFC 5246 https://datatracker.ietf.org/doc/html/rfc5246#section-6.2.1
pub const DEFAULT_BUFFER_SIZE: usize = 16 * 1024; // 16KB

// The admin thread (control plane) sessions use a fixed upper-bound on the
// session buffer size. The max buffer size for data plane sessions are to be
// specified during `Listener` initialization. This allows protocol and config
// specific upper bounds.
const ADMIN_MAX_BUFFER_SIZE: usize = 2 * 1024 * 1024; // 1MB

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const THREAD_PREFIX: &str = "pelikan";
const QUEUE_CAPACITY: usize = 64 * 1024;

pub const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
pub const WAKER_TOKEN: Token = Token(usize::MAX);

pub struct TrackedSession<Parser, Request, Response> {
    pub session: ClientSession<Parser, Request, Response>,
    pub sender: Option<usize>,
    pub token: Option<Token>,
}

impl<Parser, Request, Response> Debug for TrackedSession<Parser, Request, Response> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.session)
    }
}

impl<Parser, Request, Response> ::net::event::Source for TrackedSession<Parser, Request, Response> {
    fn register(&mut self, registry: &net::Registry, token: net::Token, interest: net::Interest) -> std::result::Result<(), std::io::Error> {
        self.session.register(registry, token, interest)
    }
    fn reregister(&mut self, registry: &net::Registry, token: net::Token, interest: net::Interest) -> std::result::Result<(), std::io::Error> {
        self.session.reregister(registry, token, interest)
    }
    fn deregister(&mut self, registry: &net::Registry) -> std::result::Result<(), std::io::Error> {
        self.session.deregister(registry)
    }
}

impl<Parser, Request, Response> TrackedSession<Parser, Request, Response>
where
    Request: Compose,
    Parser: Parse<Response>,
{
    pub fn flush(&mut self) -> Result<()> {
        self.session.flush()
    }

    pub fn interest(&self) -> Interest {
        self.session.interest()
    }

    pub fn fill(&mut self) -> Result<usize> {
        self.session.fill()
    }
}

#[derive(PartialEq, Copy, Clone, Eq, Debug)]
pub enum ConnectionState {
    Open,
    HalfClosed,
}

pub struct ClientConnection {
    addr: SocketAddr,
    stream: TcpStream,
    r_buf: Box<[u8]>,
    state: ConnectionState,
    pipeline_depth: usize,
}

impl ClientConnection {
    #[allow(clippy::slow_vector_initialization)]
    pub fn new(addr: SocketAddr, stream: TcpStream) -> Self {
        let mut r_buf = Vec::with_capacity(16384);
        r_buf.resize(16384, 0);
        let r_buf = r_buf.into_boxed_slice();

        Self {
            addr,
            stream,
            r_buf,
            state: ConnectionState::Open,
            pipeline_depth: 0,
        }
    }

    pub fn do_read(&mut self) -> Result<usize> {
        self.stream.read(&mut self.r_buf)
    }
}

pub struct TokenWrapper<T> {
    inner: T,
    token: Token,
}

impl<T> TokenWrapper<T> {
    pub fn new(inner: T, token: Token) -> Self {
        Self { inner, token }
    }

    pub fn token(&self) -> Token {
        self.token
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}
