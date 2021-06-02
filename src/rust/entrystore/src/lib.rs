// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A collection of storage datastructures suitable for use within Pelikan. A
//! typical storage module will implement one or more storage protocol traits in
//! addition to the base `Storage` trait. For example [`SegCache`] implements
//! both [`Storage`] and [`protocol::memcache::MemcacheStorage`].

mod noop;
mod seg;

pub use self::noop::*;
pub use self::seg::*;

/// A trait defining the basic requirements of a type which may be used for
/// storage.
pub trait EntryStore {
    /// Eager expiration of items/values from storage. Not all storage types
    /// will be able to efficiently implement this function. The default
    /// implementation is a no-op. Types which can efficiently implement eager
    /// expiration should implement their own handling logic for this function.
    fn expire(&mut self) {}
}
