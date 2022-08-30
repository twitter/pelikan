// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

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
use logger::Drain;
use protocol_common::{Compose, Execute, Parse};
use queues::Queues;
use rustcommon_metrics::*;
use session::{Buf, ServerSession, Session};
use slab::Slab;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

mod listener;
mod process;
mod workers;

use listener::ListenerBuilder;
use workers::WorkersBuilder;

pub use process::{Process, ProcessBuilder};

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const QUEUE_CAPACITY: usize = 64 * 1024;

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
