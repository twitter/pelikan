// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// A pre-allocated key-value store with eager expiration. It uses a
/// segment-structured design that stores data in fixed-size segments, grouping
/// objects with nearby expiration time into the same segment, and lifting most
/// per-object metadata into the shared segment header.
pub struct SegCache {
    pub(crate) hashtable: HashTable,
    pub(crate) segments: Segments,
    pub(crate) ttl_buckets: TtlBuckets,
}

impl SegCache {
    /// Returns a new `Builder` which is used to configure and construct a
    /// `SegCache` instance.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Gets a count of items in the `SegCache` instance. This is an expensive
    /// operation and is only enabled for tests and builds with the `debug`
    /// feature enabled.
    #[cfg(any(test, feature = "debug"))]
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

    pub fn insert<'a>(
        &mut self,
        key: &'a [u8],
        value: &[u8],
        optional: Option<&[u8]>,
        ttl: CoarseDuration,
    ) -> Result<(), SegCacheError<'a>> {
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
                Err(TtlBucketsError::ItemOversized { size }) => {
                    return Err(SegCacheError::ItemOversized { size, key });
                }
                Err(TtlBucketsError::NoFreeSegments) => {
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
    pub fn cas<'a>(
        &mut self,
        key: &'a [u8],
        value: &[u8],
        optional: Option<&[u8]>,
        ttl: CoarseDuration,
        cas: u32,
    ) -> Result<(), SegCacheError<'a>> {
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
    #[cfg(feature = "dump")]
    pub fn dump(&mut self) -> SegCacheDump {
        SegCacheDump {
            ttl_buckets: self.ttl_buckets.dump(),
            segments: self.segments.dump(),
        }
    }

    /// Checks the integrity of all segments
    /// *NOTE*: this operation is relatively expensive
    #[cfg(feature = "debug")]
    pub fn check_integrity(&mut self) -> Result<(), SegCacheError> {
        if self.segments.check_integrity() {
            Ok(())
        } else {
            Err(SegCacheError::DataCorrupted)
        }
    }
}
