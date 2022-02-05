// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! No-op storage which can be used for servers which do not have state.

use crate::EntryStore;

mod ping;

#[derive(Default)]
/// A no-op storage backend which implements `EntryStore` and storage protocol
/// traits.
pub struct Noop {}

impl Noop {
    /// Create a new `Noop` storage backend
    pub fn new() -> Self {
        Noop::default()
    }
}

impl EntryStore for Noop {
    fn expire(&mut self) {}
}
