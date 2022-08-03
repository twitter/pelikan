// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#![allow(dead_code)]
#![allow(unused_imports)]

#[macro_use]
extern crate logger;

use admin::AdminBuilder;
use ::net::event::{Event, Source};
use ::net::*;
use common::signal::Signal;
use common::ssl::tls_acceptor;
use config::*;
use config::proxy::*;
use core::marker::PhantomData;
use core::time::Duration;
use crossbeam_channel::{bounded, Receiver, Sender};
use entrystore::EntryStore;
use logger::Drain;
use protocol_common::{Compose, Execute, Parse};
use queues::Queues;
use session_common::{Buf, ServerSession, Session};
use slab::Slab;
use std::io::{Error, ErrorKind, Result};
use std::sync::Arc;

// mod admin;
mod backend;
mod frontend;
mod listener;
mod process;


use backend::BackendBuilder;
use frontend::FrontendBuilder;
use listener::ListenerBuilder;

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

fn map_result(result: Result<usize>) -> Result<()> {
    match result {
        Ok(0) => Err(Error::new(ErrorKind::Other, "client hangup")),
        Ok(_) => Ok(()),
        Err(e) => map_err(e),
    }
}
