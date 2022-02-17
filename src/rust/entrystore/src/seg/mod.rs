// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segment-structured storage which implements efficient proactive eviction.
//! This storage type is suitable for use in simple key-value cache backends.
//! See: [`::seg`] crate for more details behind the underlying storage design.

use crate::EntryStore;

use config::seg::Eviction;
use config::SegConfig;
use seg::{Policy, SegError, Value};

mod memcache;

/// A wrapper around [`seg::Seg`] which implements `EntryStore` and storage
/// protocol traits.
pub struct Seg {
    data: ::seg::Seg,
}

impl Seg {
    /// Create `Seg` storage based on the config and the `TimeType` which is
    /// used to interpret various expiry time formats.
    pub fn new<T: SegConfig>(config: &T) -> Self {
        let config = config.seg();

        // build up the eviction policy from the config
        let eviction = match config.eviction() {
            Eviction::None => Policy::None,
            Eviction::Random => Policy::Random,
            Eviction::RandomFifo => Policy::RandomFifo,
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
        let data = ::seg::Seg::builder()
            .restore(config.restore())
            .hash_power(config.hash_power())
            .overflow_factor(config.overflow_factor())
            .heap_size(config.heap_size())
            .segment_size(config.segment_size())
            .eviction(eviction)
            .datapool_path(config.datapool_path())
            .segments_fields_path(config.segments_fields_path())
            .ttl_buckets_path(config.ttl_buckets_path())
            .hashtable_path(config.hashtable_path())
            .build();

        Self { data }
    }

    /// Demolish (gracefully shutdown) the cache if
    /// configured to do so
    pub fn demolish<T: SegConfig>(self, config: &T) {
        let config = config.seg();

        if config.graceful_shutdown() {
            ::seg::Seg::demolisher()
                .heap_size(config.heap_size())
                .overflow_factor(config.overflow_factor())
                .segments_fields_path(config.segments_fields_path())
                .ttl_buckets_path(config.ttl_buckets_path())
                .hashtable_path(config.hashtable_path())
                .demolish(self.data);
        };
    }
}

impl EntryStore for Seg {
    fn expire(&mut self) {
        self.data.expire();
    }
}
