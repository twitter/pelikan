// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Defines the `Memcache` storage interface and implements the wire protocol.

mod entry;
mod storage;
mod wire;

pub use entry::MemcacheEntry;
pub use storage::{MemcacheStorage, MemcacheStorageError};
pub use wire::{MemcacheRequest, MemcacheRequestParser, MemcacheResponse, MemcacheResult};

#[cfg(feature = "stats")]
use common::time::Nanoseconds;
use common::time::Seconds;

pub(crate) type UnixInstant = common::time::UnixInstant<Seconds<u32>>;

#[cfg(feature = "stats")]
pub(crate) type PreciseDuration = common::time::Duration<Nanoseconds<u64>>;
#[cfg(feature = "stats")]
pub(crate) type PreciseInstant = common::time::Instant<Nanoseconds<u64>>;
