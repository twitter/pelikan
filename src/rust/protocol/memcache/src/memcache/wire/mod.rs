// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Implements the wire protocol for the `Memcache` protocol implementation.

mod request;
mod response;

pub use request::*;
pub use response::*;

#[cfg(feature = "stats")]
mod metrics;

#[cfg(feature = "stats")]
pub use metrics::*;
