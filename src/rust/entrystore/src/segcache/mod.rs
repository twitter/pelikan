// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segment-structured storage which implements efficient proactive eviction.
//! This storage type is suitable for use in simple key-value cache backends.
//! See: [`::segcache`] crate for more details behind the underlying storage
//! design.

use crate::EntryStore;

use config::segcache::Eviction;
use config::{SegCacheConfig, TimeType};
use rustcommon_time::CoarseDuration;
use segcache::{Policy, SegCacheError};

use std::time::SystemTime;

mod memcache;

/// A wrapper around [`segcache::SegCache`] which implements `EntryStore` and
/// storage protocol traits.
pub struct SegCache {
    data: ::segcache::SegCache,
    time_type: TimeType,
}

impl SegCache {
    /// Create a new `SegCache` based on the config and the `TimeType` which is
    /// used to interpret various expiry time formats.
    pub fn new(config: &SegCacheConfig, time_type: TimeType) -> Self {
        // build up the eviction policy from the config
        let eviction = match config.eviction() {
            Eviction::None => Policy::None,
            Eviction::Random => Policy::Random,
            Eviction::Fifo => Policy::Fifo,
            Eviction::Cte => Policy::Cte,
            Eviction::Util => Policy::Util,
            Eviction::Merge => Policy::Merge {
                max: config.merge_max(),
                merge: config.merge_target(),
                compact: config.compact_target(),
            },
        };

        // build the datastructure from the config
        let data = ::segcache::SegCache::builder()
            .power(config.hash_power())
            .overflow_factor(config.overflow_factor())
            .heap_size(config.heap_size())
            .segment_size(config.segment_size())
            .eviction(eviction)
            .datapool_path(config.datapool_path())
            .build();

        Self { data, time_type }
    }

    // TODO(bmartin): should this be moved up into a common function?
    // TODO(bmartin): can we use coarse time for the conversion?
    /// Internal function which converts an expiry time into a TTL in seconds.
    fn get_ttl(&self, expiry: u32) -> u32 {
        match self.time_type {
            TimeType::Unix => {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                expiry.wrapping_sub(epoch)
            }
            TimeType::Delta => expiry,
            TimeType::Memcache => {
                if expiry < 60 * 60 * 24 * 30 {
                    expiry
                } else {
                    let epoch = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as u32;
                    expiry.wrapping_sub(epoch)
                }
            }
        }
    }
}

impl EntryStore for SegCache {
    fn expire(&mut self) {
        self.data.expire();
    }
}
