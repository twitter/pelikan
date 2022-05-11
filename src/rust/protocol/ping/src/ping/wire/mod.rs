// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the wire protocol for the `Ping` protocol implementation.

mod request;
mod response;

pub use request::*;
pub use response::*;

use common::metrics::metric;
#[allow(unused_imports)]
use metrics::Counter;

#[cfg(feature = "server")]
#[metric(name="ping", crate=common::metrics)]
static PING: Counter = Counter::new();

#[cfg(feature = "client")]
#[metric(name="pong", crate=common::metrics)]
static PONG: Counter = Counter::new();
