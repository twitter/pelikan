// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod connector;
mod listener;
mod stream;
mod tcp;
mod tls_tcp;

pub use connector::*;
pub use listener::*;
pub use stream::*;
pub use tcp::*;
pub use tls_tcp::*;

pub mod event {
    pub use mio::event::*;
}

pub use mio::*;

use core::fmt::Debug;
use core::ops::Deref;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::{Path, PathBuf};

use foreign_types_shared::{ForeignType, ForeignTypeRef};
use rustcommon_metrics::*;

type Result<T> = std::io::Result<T>;
