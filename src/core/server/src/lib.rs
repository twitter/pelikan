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

use admin::{Admin, AdminBuilder};
use listener::{Listener, ListenerBuilder};
use workers::{Workers, WorkersBuilder};

pub use process::{Process, ProcessBuilder};

// TODO(bmartin): this *should* be plenty safe, the queue should rarely ever be
// full, and a single wakeup should drain at least one message and make room for
// the response. A stat to prove that this is sufficient would be good.
const QUEUE_RETRIES: usize = 3;

const QUEUE_CAPACITY: usize = 64 * 1024;

const LISTENER_TOKEN: Token = Token(usize::MAX - 1);
const WAKER_TOKEN: Token = Token(usize::MAX);

const THREAD_PREFIX: &str = "pelikan";
