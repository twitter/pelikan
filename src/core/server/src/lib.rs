// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate defines a Pelikan cache server which is a single process with
//! multiple threads.
//!
//! # Thread Model
//! The Pelikan cache servers are comprised of multiple threads which allow us
//! to separate the control and data planes. There are two distinct thread
//! models, one which is used for a single worker thread and then a expanded
//! model when multiple workers are configured. For a single worker we
//! specialize by leaving the cache storage as thread-local. For multiple
//! workers, the cache storage is handled in its own `storage` thread.
//!
//! Single worker thread model:
//! ```text
//! ┌──────────┐         ┌──────────┐
//! │  admin   │         │ listener │
//! │          │         │          │
//! │  :9999   │         │  :12321  │
//! └──────────┘         └──────────┘
//!                            │
//!       ┌────────────────────┘
//!       │
//!       ▼
//! ┌──────────┐        ┌ ─ ─ ─ ─ ─
//! │          │                   │─
//! │  worker  │◀──────▶│  client    │─
//! │          │                   │   │
//! └──────────┘        └ ─ ─ ─ ─ ─  │
//!                       └ ─ ─ ─ ─ ─  │
//!                         └ ─ ─ ─ ─ ─
//! ```
//!
//! Multiple worker thread model:
//! ```text
//! ┌──────────┐                 ┌──────────┐
//! │  admin   │                 │ listener │
//! │          │                 │          │
//! │  :9999   │                 │  :12321  │
//! └──────────┘                 └──────────┘
//!                                    │
//!                            ┌───────┴───────┐
//!                            │               │
//!                            ▼               ▼
//! ┌ ─ ─ ─ ─ ─          ┌──────────┐    ┌──────────┐        ┌ ─ ─ ─ ─ ─
//!            │─        │          │    │          │                   │─
//! │  client   ◀┼──────▶│  worker  │    │  worker  │◀──────▶│  client    │─
//!            │   │     │          │    │          │                   │   │
//! └ ─ ─ ─ ─ ─  │       └──────────┘    └──────────┘        └ ─ ─ ─ ─ ─  │
//!   └ ─ ─ ─ ─ ─  │           ▲               ▲               └ ─ ─ ─ ─ ─  │
//!     └ ─ ─ ─ ─ ─            └───────┬───────┘                 └ ─ ─ ─ ─ ─
//!                                    ▼
//!                              ┌──────────┐
//!                              │          │
//!                              │ storage  │
//!                              │          │
//!                              └──────────┘
//! ```
//!
//! ## Control Plane
//! The control plane is handled by a single `admin` thread. This thread is
//! responsible for handling administrative commands and metrics exposition.
//!
//! ## Data Plane
//! The data plane is handled by two or more threads depending on the
//! configuration.
//!
//! ### Listener
//! At a minimum we have one `listener` thread which owns the listening socket,
//! accepts new connections, and handles TLS negotiation if it is enabled by the
//! configuration. Fully negotiated sessions are then handed off to one or more
//! worker threads.
//!
//! ### Worker
//! Worker threads handle ongoing communications for an established session.
//! This includes request parsing and response composition. If the configuration
//! specifies a single worker thread, the thread also owns the underlying cache
//! datastructure. However, if the configuration specifies multiple worker
//! threads a fully parsed request is handed over to the `storage` thread for
//! execution.
//!
//! ### Storage
//! An optional thread which is used only if there is more than one worker
//! thread configured. This thread is used to own the cache datastructure and
//! execute requests. The storage thread will receive requests from a worker
//! over a queue, execute the request, and returns the result back to the worker
//! thread.

#[macro_use]
extern crate logger;

use ::net::event::{Event, Source};
use ::net::*;
use admin::AdminBuilder;
use common::signal::Signal;
use common::ssl::tls_acceptor;
use config::*;
use core::marker::PhantomData;
use core::time::Duration;
use crossbeam_channel::{bounded, Sender};
use entrystore::EntryStore;
use logger::{Drain, Klog};
use protocol_common::{Compose, Execute, Parse};
use queues::Queues;
use rustcommon_metrics::*;
use session::{Buf, ServerSession, Session};
use slab::Slab;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;
use waker::Waker;

mod listener;
mod process;
mod workers;

use listener::ListenerBuilder;
use workers::WorkersBuilder;

pub use process::{Process, ProcessBuilder};

type Instant = rustcommon_metrics::Instant<rustcommon_metrics::Nanoseconds<u64>>;

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const QUEUE_CAPACITY: usize = 64 * 1024;

// determines the max number of calls to accept when the listener is ready
const ACCEPT_BATCH: usize = 8;

const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
const WAKER_TOKEN: Token = Token(usize::MAX);

const THREAD_PREFIX: &str = "pelikan";

pub static PERCENTILES: &[(&str, f64)] = &[
    ("p25", 25.0),
    ("p50", 50.0),
    ("p75", 75.0),
    ("p90", 90.0),
    ("p99", 99.0),
    ("p999", 99.9),
    ("p9999", 99.99),
];

// stats
counter!(PROCESS_REQ);

fn map_err(e: std::io::Error) -> Result<()> {
    match e.kind() {
        ErrorKind::WouldBlock => Ok(()),
        _ => Err(e),
    }
}

common::metrics::test_no_duplicates!();
