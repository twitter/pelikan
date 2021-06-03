// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segment-structured storage which implements efficient proactive eviction.
//! This storage type is suitable for use in simple key-value cache backends.
//! See: [`::segcache`] crate for more details behind the underlying storage
//! design.

use crate::EntryStore;

use config::seg::Eviction;
use config::{SegConfig, TimeType};
use rustcommon_time::CoarseDuration;
use seg::{Policy, SegError};

mod memcache;

/// A wrapper around [`seg::Seg`] which implements `EntryStore` and storage
/// protocol traits.
pub struct Seg {
    data: ::seg::Seg,
    time_type: TimeType,
}

impl Seg {
    /// Create a new `SegCache` based on the config and the `TimeType` which is
    /// used to interpret various expiry time formats.
    pub fn new(config: &SegConfig, time_type: TimeType) -> Self {
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
        let data = ::seg::Seg::builder()
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
    /// Internal function which converts an expiry time into a TTL in seconds.
    fn get_ttl(&self, expiry: u32) -> Option<u32> {
        if self.time_type == TimeType::Unix
            || (self.time_type == TimeType::Memcache && expiry >= 60 * 60 * 24 * 30)
        {
            expiry.checked_sub(rustcommon_time::recent_unix())
        } else {
            Some(expiry)
        }
    }
}

impl EntryStore for Seg {
    fn expire(&mut self) {
        self.data.expire();
    }
}
