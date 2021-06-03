// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segment-structured storage which implements efficient proactive eviction.
//! This storage type is suitable for use in simple key-value cache backends.
//! See: [`::segcache`] crate for more details behind the underlying storage
//! design.

use crate::EntryStore;

mod ping;

/// A no-op storage backend which implements `EntryStore` and storage protocol
/// traits.
pub struct Noop {}

impl Default for Noop {
    fn default() -> Self {
        Noop {}
    }
}

impl Noop {
    /// Create a new `Noop` storage backend
    pub fn new() -> Self {
        Noop::default()
    }
}

impl EntryStore for Noop {
    fn expire(&mut self) {}
}
