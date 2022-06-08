// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Defines the `Memcache` storage interface and implements the wire protocol.

mod entry;
mod storage;
mod wire;

pub use entry::MemcacheEntry;
pub use storage::{MemcacheStorage, MemcacheStorageError};
pub use wire::*;

#[cfg(feature = "stats")]
use rustcommon_metrics::Nanoseconds;

#[cfg(feature = "stats")]
pub(crate) type PreciseInstant = rustcommon_metrics::Instant<Nanoseconds<u64>>;
