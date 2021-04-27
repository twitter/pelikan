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

// includes from other crates
use rustcommon_time::*;
use thiserror::Error;

// includes from core/std
use core::hash::BuildHasher;
use core::hash::Hasher;
use std::convert::TryInto;

// submodules
mod common;
mod hashtable;
mod item;
mod rand;
mod segments;
mod ttl_buckets;

// publicly exported items from submodules
pub use item::Item;
pub use segments::Policy;

// items from submodules which are imported for convenience to the crate level
pub(crate) use crate::rand::*;
pub(crate) use common::*;
pub(crate) use hashtable::*;
pub(crate) use item::*;
pub(crate) use metrics::*;
pub(crate) use segments::*;
pub(crate) use ttl_buckets::*;

/// `SegCache` is the main datastructure. It is a pre-allocated key-value store
/// with eager expiration. It uses a segment-structured design that stores data
/// in fixed-size segments, grouping objects with nearby expiration time into
/// the same segment, and lifting most per-object metadata into the shared
/// segment header.
pub struct SegCache<S: std::hash::BuildHasher> {
    hashtable: HashTable<S>,
    segments: Segments,
    ttl_buckets: TtlBuckets,
}

#[derive(Error, Debug)]
/// Possible errors returned by the top-level `SegCache` API
pub enum SegCacheError {
    #[error("hashtable insert exception")]
    HashTableInsertEx,
    #[error("eviction exception")]
    EvictionEx,
    #[error("item oversized")]
    ItemOversized,
    #[error("no free segments")]
    NoFreeSegments,
    #[error("item exists")]
    Exists,
    #[error("item not found")]
    NotFound,
    #[error("data corruption detected")]
    DataCorrupted,
}

/// A `Builder` is used to construct a new `SegCache` instance.
pub struct Builder<S: std::hash::BuildHasher> {
    hasher: Option<S>,
    power: u8,
    overflow_factor: f64,
    segments_builder: SegmentsBuilder,
}

// Defines the default parameters
impl<S: std::hash::BuildHasher> Default for Builder<S> {
    fn default() -> Self {
        Self {
            hasher: None,
            power: 16,
            overflow_factor: 0.0,
            segments_builder: SegmentsBuilder::default(),
        }
    }
}

impl<S: std::hash::BuildHasher + Default> Builder<S> {
    /// Provide a `BuildHasher` type which is used to calculate hash values for
    /// the keys.
    pub fn hasher(mut self, hasher: S) -> Self {
        self.hasher = Some(hasher);
        self
    }

    /// Specify the hash power, which limits the size of the hashtable to 2^N
    /// entries. 1/8th of these are used for metadata storage, meaning that the
    /// total number of items which can be held in the cache is limited to
    /// `7 * 2^(N - 3)` items. The hash table will have a total size of
    /// `2^(N + 3)` bytes.
    ///
    ///
    pub fn power(mut self, power: u8) -> Self {
        assert!(power >= 3, "power must be at least 3");
        self.power = power;
        self
    }

    /// Specify an overflow factor which is used to scale the hashtable and
    /// provide additional capacity for chaining item buckets. A factor of 1.0
    /// will result in a hash table that is 100% larger.
    pub fn overflow_factor(mut self, percent: f64) -> Self {
        self.overflow_factor = percent;
        self
    }

    /// Specify the total number of bytes to be used for heap storage of items.
    /// This includes, key, value, and per-item overheads.
    pub fn heap_size(mut self, bytes: usize) -> Self {
        self.segments_builder = self.segments_builder.heap_size(bytes);
        self
    }

    /// Specify the segment size for item storage. The largest item which can be
    /// held is `size - 5` bytes for builds without the `debug` or `magic` build
    /// features enabled. Smaller segment sizes reduce the number of items which
    /// would be evicted/expired at one time, at the cost of additional memory
    /// and book-keeping overheads compared to using larger segments for the
    /// same total size.
    pub fn segment_size(mut self, size: i32) -> Self {
        self.segments_builder = self.segments_builder.segment_size(size);
        self
    }

    /// Specify the eviction policy to be used. See the `Policy` documentation
    /// for more details about each strategy.
    pub fn eviction(mut self, policy: Policy) -> Self {
        self.segments_builder = self.segments_builder.eviction_policy(policy);
        self
    }

    /// Consumes the builder and returns a fully-allocated `SegCache` instance.
    pub fn build(self) -> SegCache<S> {
        let hasher = self.hasher.unwrap_or_default();

        let hashtable = HashTable::with_hasher(self.power, self.overflow_factor, hasher);

        let segments = self.segments_builder.build();
        let ttl_buckets = TtlBuckets::default();
        SegCache {
            hashtable,
            segments,
            ttl_buckets,
        }
    }
}

impl<S: std::hash::BuildHasher> SegCache<S> {
    /// Returns a new `Builder` which is used to configure and construct a
    /// `SegCache` instance.
    pub fn builder() -> Builder<S> {
        Builder::default()
    }

    /// Gets a count of items in the `SegCache` instance.
    // TODO(bmartin): rename to len() ?
    pub fn items(&mut self) -> usize {
        trace!("getting segment item counts");
        self.segments.items()
    }

    /// Get the item in the `SegCache` with the provided key
    pub fn get(&mut self, key: &[u8]) -> Option<Item> {
        self.hashtable.get(key, &mut self.segments)
    }

    /// Get the item in the `SegCache` with the provided key without
    /// increasing the item frequency - useful for combined operations that
    /// check for presence - eg replace is a get + set
    pub fn get_no_freq_incr(&mut self, key: &[u8]) -> Option<Item> {
        self.hashtable.get_no_freq_incr(key, &mut self.segments)
    }

    pub fn insert(
        &mut self,
        key: &[u8],
        value: &[u8],
        optional: Option<&[u8]>,
        ttl: CoarseDuration,
    ) -> Result<(), SegCacheError> {
        // default optional data is empty
        let optional = optional.unwrap_or(&[]);

        // calculate size for item
        let size = (((ITEM_HDR_SIZE + key.len() + value.len() + optional.len()) >> 3) + 1) << 3;

        // try to get a `ReservedItem`
        let mut retries = 3;
        let reserved;
        loop {
            match self
                .ttl_buckets
                .get_mut_bucket(ttl)
                .reserve(size, &mut self.segments)
            {
                Ok(mut reserved_item) => {
                    reserved_item.define(key, value, optional);
                    reserved = reserved_item;
                    break;
                }
                Err(ttl_buckets::Error::ItemOversized) => {
                    return Err(SegCacheError::ItemOversized);
                }
                Err(ttl_buckets::Error::NoFreeSegments) => {
                    if self
                        .segments
                        .evict(&mut self.ttl_buckets, &mut self.hashtable)
                        .is_err()
                    {
                        retries -= 1;
                    } else {
                        continue;
                    }
                }
            }
            if retries == 0 {
                return Err(SegCacheError::NoFreeSegments);
            }
            retries -= 1;
        }

        // insert into the hashtable, or roll-back by removing the item
        // TODO(bmartin): we can probably roll-back the offset and re-use the
        // space in the segment, currently we consume the space even if the
        // hashtable is overfull
        if self
            .hashtable
            .insert(
                reserved.item(),
                reserved.seg(),
                reserved.offset() as u64,
                &mut self.ttl_buckets,
                &mut self.segments,
            )
            .is_err()
        {
            let _ = self.segments.remove_at(
                reserved.seg(),
                reserved.offset(),
                false,
                &mut self.ttl_buckets,
                &mut self.hashtable,
            );
            Err(SegCacheError::HashTableInsertEx)
        } else {
            Ok(())
        }
    }

    /// Performs a CAS operation, inserting the item only if the CAS value
    /// matches the current value for that item.
    pub fn cas(
        &mut self,
        key: &[u8],
        value: &[u8],
        optional: Option<&[u8]>,
        ttl: CoarseDuration,
        cas: u32,
    ) -> Result<(), SegCacheError> {
        match self.hashtable.try_update_cas(key, cas, &mut self.segments) {
            Ok(()) => self.insert(key, value, optional, ttl),
            Err(e) => Err(e),
        }
    }

    /// Remove the item with the given key, returns a bool indicating if it was
    /// removed.
    // TODO(bmartin): a result would be better here
    pub fn delete(&mut self, key: &[u8]) -> bool {
        self.hashtable
            .delete(key, &mut self.ttl_buckets, &mut self.segments)
    }

    /// Loops through the TTL Buckets to handle eager expiration, returns the
    /// number of segments expired
    pub fn expire(&mut self) -> usize {
        rustcommon_time::refresh_clock();
        self.ttl_buckets
            .expire(&mut self.hashtable, &mut self.segments)
    }

    /// Produces a dump of the cache for analysis
    /// *NOTE*: this operation is relatively expensive
    pub fn dump(&mut self) -> SegCacheDump {
        SegCacheDump {
            ttl_buckets: self.ttl_buckets.dump(),
            segments: self.segments.dump(),
        }
    }

    /// Checks the integrity of all segments
    /// *NOTE*: this operation is relatively expensive
    pub fn check_integrity(&mut self) -> Result<(), SegCacheError> {
        if self.segments.check_integrity() {
            Ok(())
        } else {
            Err(SegCacheError::DataCorrupted)
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SegCacheDump {
    ttl_buckets: Vec<TtlBucketDump>,
    segments: Vec<SegmentDump>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::ITEM_HDR_SIZE;
    use ahash::RandomState;

    // Use ahash for testing with fixed seeds. Reproducible testing is important
    fn build_hasher() -> RandomState {
        RandomState::with_seeds(
            0xbb8c484891ec6c86,
            0x0522a25ae9c769f9,
            0xeed2797b9571bc75,
            0x4feb29c1fbbd59d0,
        )
    }

    #[test]
    fn sizes() {
        #[cfg(feature = "magic")]
        assert_eq!(ITEM_HDR_SIZE, 9);

        #[cfg(not(feature = "magic"))]
        assert_eq!(ITEM_HDR_SIZE, 5);

        assert_eq!(std::mem::size_of::<Segments>(), 64);
        assert_eq!(std::mem::size_of::<SegmentHeader>(), 64);

        assert_eq!(std::mem::size_of::<HashBucket>(), 64);
        assert_eq!(std::mem::size_of::<HashTable<ahash::RandomState>>(), 64);

        assert_eq!(std::mem::size_of::<crate::ttl_buckets::TtlBucket>(), 64);
        assert_eq!(std::mem::size_of::<TtlBuckets>(), 16);
    }

    #[test]
    fn init() {
        let mut cache = SegCache::builder()
            .segment_size(4096)
            .heap_size(4096 * 64)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
    }

    #[test]
    fn get_free_seg() {
        let segment_size = 4096;
        let segments = 64;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 64);
        let seg = cache.segments.pop_free();
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(seg, Some(0));
    }

    #[test]
    fn get() {
        let ttl = CoarseDuration::ZERO;
        let segment_size = 4096;
        let segments = 64;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 64);
        assert!(cache.get(b"coffee").is_none());
        assert!(cache.insert(b"coffee", b"strong", None, ttl).is_ok());
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 1);
        assert!(cache.get(b"coffee").is_some());

        let item = cache.get(b"coffee").unwrap();
        assert_eq!(item.value(), b"strong", "item is: {:?}", item);
    }

    #[test]
    fn overwrite() {
        let ttl = CoarseDuration::ZERO;
        let segment_size = 4096;
        let segments = 64;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 64);
        assert!(cache.get(b"drink").is_none());

        println!("==== first insert ====");
        assert!(cache.insert(b"drink", b"coffee", None, ttl).is_ok());
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 1);
        let item = cache.get(b"drink");
        assert!(item.is_some());
        let item = item.unwrap();
        let value = item.value();
        assert_eq!(value, b"coffee", "item is: {:?}", item);

        println!("==== second insert ====");
        assert!(cache.insert(b"drink", b"espresso", None, ttl).is_ok());
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 1);
        let item = cache.get(b"drink");
        assert!(item.is_some());
        let item = item.unwrap();
        let value = item.value();
        assert_eq!(value, b"espresso", "item is: {:?}", item);

        println!("==== third insert ====");
        assert!(cache.insert(b"drink", b"whisky", None, ttl).is_ok());
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 1);
        let item = cache.get(b"drink");
        assert!(item.is_some());
        let item = item.unwrap();
        let value = item.value();
        assert_eq!(value, b"whisky", "item is: {:?}", item);
    }

    #[test]
    fn delete() {
        let ttl = CoarseDuration::ZERO;
        let segment_size = 4096;
        let segments = 64;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 64);
        assert!(cache.get(b"drink").is_none());

        assert!(cache.insert(b"drink", b"coffee", None, ttl).is_ok());
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 1);
        let item = cache.get(b"drink");
        assert!(item.is_some());
        let item = item.unwrap();
        let value = item.value();
        assert_eq!(value, b"coffee", "item is: {:?}", item);

        assert_eq!(cache.delete(b"drink"), true);
        assert_eq!(cache.segments.free(), 63);
        assert_eq!(cache.items(), 0);
    }

    #[test]
    fn collisions_2() {
        let ttl = CoarseDuration::ZERO;
        let segment_size = 64;
        let segments = 2;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .power(3)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 2);

        // note: we can only fit 7 because the first bucket in the chain only
        // has 7 slots. since we don't support chaining, we must have a
        // collision on the 8th insert.
        for i in 0..1000 {
            let i = i % 3;
            let v = format!("{:02}", i);
            assert!(cache.insert(v.as_bytes(), v.as_bytes(), None, ttl).is_ok());
            let item = cache.get(v.as_bytes());
            assert!(item.is_some());
        }
    }

    #[test]
    fn collisions() {
        let ttl = CoarseDuration::ZERO;
        let segment_size = 4096;
        let segments = 64;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .power(3)
            .hasher(build_hasher())
            .build();
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), 64);

        // note: we can only fit 7 because the first bucket in the chain only
        // has 7 slots. since we don't support chaining, we must have a
        // collision on the 8th insert.
        for i in 0..7 {
            let v = format!("{}", i);
            assert!(cache.insert(v.as_bytes(), v.as_bytes(), None, ttl).is_ok());
            let item = cache.get(v.as_bytes());
            assert!(item.is_some());
            assert_eq!(cache.items(), i + 1);
        }
        let v = b"8";
        assert!(cache.insert(v, v, None, ttl).is_err());
        assert_eq!(cache.items(), 7);
        assert_eq!(cache.delete(b"0"), true);
        assert_eq!(cache.items(), 6);
        assert!(cache.insert(v, v, None, ttl).is_ok());
        assert_eq!(cache.items(), 7);
    }

    #[test]
    fn full_cache_long() {
        let ttl = CoarseDuration::ZERO;
        let iters = 1_000_000;
        let segments = 32;
        let segment_size = 1024;
        let key_size = 1;
        let value_size = 512;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .power(16)
            .hasher(build_hasher())
            .build();

        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), segments);

        let mut rng = rand::rng();

        let mut key = vec![0; key_size];
        let mut value = vec![0; value_size];

        let mut inserts = 0;

        for _ in 0..iters {
            rng.fill_bytes(&mut key);
            rng.fill_bytes(&mut value);

            if cache.insert(&key, &value, None, ttl).is_ok() {
                inserts += 1;
            };
        }

        assert_eq!(inserts, iters);
    }

    #[test]
    fn full_cache_long_2() {
        let ttl = CoarseDuration::ZERO;
        let iters = 10_000_000;
        let segments = 64;
        let segment_size = 2 * 1024;
        let key_size = 2;
        let value_size = 1;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .power(16)
            .hasher(build_hasher())
            .build();

        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), segments);

        let mut rng = rand::rng();

        let mut key = vec![0; key_size];
        let mut value = vec![0; value_size];

        let mut inserts = 0;

        for _ in 0..iters {
            rng.fill_bytes(&mut key);
            rng.fill_bytes(&mut value);

            if cache.insert(&key, &value, None, ttl).is_ok() {
                inserts += 1;
            };
        }

        #[cfg(not(feature = "magic"))]
        assert_eq!(inserts, 9999721);

        #[cfg(feature = "magic")]
        assert_eq!(inserts, 9999702);
    }

    #[test]
    fn expiration() {
        let segments = 64;
        let segment_size = 2 * 1024;
        let heap_size = segments * segment_size as usize;

        let mut cache = SegCache::builder()
            .segment_size(segment_size)
            .heap_size(heap_size)
            .power(16)
            .hasher(build_hasher())
            .build();

        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), segments);

        assert!(cache
            .insert(b"latte", b"", None, CoarseDuration::from_secs(5))
            .is_ok());
        assert!(cache
            .insert(b"espresso", b"", None, CoarseDuration::from_secs(15))
            .is_ok());

        assert!(cache.get(b"latte").is_some());
        assert!(cache.get(b"espresso").is_some());
        assert_eq!(cache.items(), 2);
        assert_eq!(cache.segments.free(), segments - 2);

        // not enough time elapsed, not removed by expire
        cache.expire();
        assert!(cache.get(b"latte").is_some());
        assert!(cache.get(b"espresso").is_some());
        assert_eq!(cache.items(), 2);
        assert_eq!(cache.segments.free(), segments - 2);

        // wait and expire again
        std::thread::sleep(std::time::Duration::from_secs(10));
        cache.expire();

        assert!(cache.get(b"latte").is_none());
        assert!(cache.get(b"espresso").is_some());
        assert_eq!(cache.items(), 1);
        assert_eq!(cache.segments.free(), segments - 1);

        // wait and expire again
        std::thread::sleep(std::time::Duration::from_secs(10));
        cache.expire();

        assert!(cache.get(b"latte").is_none());
        assert!(cache.get(b"espresso").is_none());
        assert_eq!(cache.items(), 0);
        assert_eq!(cache.segments.free(), segments);
    }
}
