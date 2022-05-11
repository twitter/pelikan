// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! TTL buckets are used to group items by TTL to enable eager expiration.
//!
//! The total collection of [`TtlBuckets`] is a contiguous allocation of
//! [`TtlBucket`]s which cover the full range of TTLs.
//!
//! Each [`TtlBucket`] contains a segment chain holding items with a similar
//! TTL. See the blog post for more details on the segcache design:
//! <https://twitter.github.io/pelikan/2021/segcache.html>
//!

mod error;
mod ttl_bucket;
#[allow(clippy::module_inception)]
mod ttl_buckets;

#[cfg(test)]
mod tests;

pub use error::TtlBucketsError;
pub use ttl_bucket::TtlBucket;
pub use ttl_buckets::TtlBuckets;

use common::metrics::metric;
use metrics::Counter;

#[metric(name="segment_clear", crate=common::metrics)]
static SEGMENT_CLEAR: Counter = Counter::new();
#[metric(name="segment_expire", crate=common::metrics)]
static SEGMENT_EXPIRE: Counter = Counter::new();

#[metric(name="clear_time", crate=common::metrics)]
static CLEAR_TIME: Counter = Counter::new();
#[metric(name="expire_time", crate=common::metrics)]
static EXPIRE_TIME: Counter = Counter::new();
