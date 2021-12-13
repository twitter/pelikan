// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This crate is a Rust implementation of the Segcache storage layer.
//!
//! It is a high-throughput and memory-efficient key-value store with eager
//! expiration. Segcache uses a segment-structured design that stores data in
//! fixed-size segments, grouping objects with nearby expiration time into the
//! same segment, and lifting most per-object metadata into the shared segment
//! header. This reduces object metadata by 88% compared to Memcached.
//!
//! A blog post about the overall design can be found here:
//! <https://twitter.github.io/pelikan/2021/segcache.html>
//!
//! Goals:
//! * high-throughput item storage
//! * eager expiration of items
//! * low metadata overhead
//!
//! Non-goals:
//! * not designed for concurrent access
//!

// macro includes
#[macro_use]
extern crate rustcommon_logger;

// external crate includes
use rustcommon_time::*;

// includes from core/std
use core::hash::{BuildHasher, Hasher};
use std::convert::TryInto;

// submodules
mod builder;
mod datapool;
mod error;
mod eviction;
mod hashtable;
mod item;
mod rand;
mod seg;
mod segments;
mod ttl_buckets;

// tests
#[cfg(test)]
mod tests;

// publicly exported items from submodules
pub use crate::seg::Seg;
pub use builder::Builder;
pub use error::SegError;
pub use eviction::Policy;
pub use item::Item;

// publicly exported items from external crates
pub use rustcommon_time::CoarseDuration;
pub use storage_types::Value;

// items from submodules which are imported for convenience to the crate level
pub(crate) use crate::rand::*;
pub(crate) use hashtable::*;
pub(crate) use item::*;
pub(crate) use segments::*;
pub(crate) use ttl_buckets::*;

metrics::test_no_duplicates!();
