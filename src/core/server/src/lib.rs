// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate logger;

use ::net::event::Event;
use ::net::event::Source;
use ::net::*;
use common::signal::Signal;
use common::ssl::tls_acceptor;
use config::*;
use core::marker::PhantomData;
use core::time::Duration;
use crossbeam_channel::bounded;
use crossbeam_channel::Receiver;
use crossbeam_channel::Sender;
use entrystore::EntryStore;
use logger::Drain;
use protocol_admin::AdminRequest;
use protocol_admin::AdminRequestParser;
use protocol_admin::AdminResponse;
use protocol_common::Compose;
use protocol_common::Execute;
use protocol_common::Parse;
use queues::Queues;
use session_common::Buf;
use session_common::ServerSession;
use session_common::Session;
use slab::Slab;
use std::io::Result;
use std::io::{Error, ErrorKind};
use std::sync::Arc;

mod admin;
mod listener;
mod process;
mod workers;

use admin::AdminBuilder;
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

fn map_err(e: std::io::Error) -> Result<()> {
    match e.kind() {
        ErrorKind::WouldBlock => Ok(()),
        _ => Err(e),
    }
}
