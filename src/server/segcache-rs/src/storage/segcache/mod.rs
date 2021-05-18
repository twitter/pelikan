// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::storage::GetTtl;
use ::segcache::Policy;
use ::segcache::SegCacheError;
use config::segcache::Eviction;
use config::SegCacheConfig;
use config::TimeType;
use metrics::*;
use rustcommon_time::CoarseDuration;
use std::time::SystemTime;

mod memcache;

pub trait Storage {
    fn expire(&mut self);
}

/// A wrapper type around `SegCache` storage crate which allows us to store
/// additional state which is not part of the storage crate.
pub struct SegCache {
    time_type: TimeType,
    data: ::segcache::SegCache,
}

impl SegCache {
    pub fn new(config: &SegCacheConfig, time_type: TimeType) -> Self {
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

        let data = ::segcache::SegCache::builder()
            .power(config.hash_power())
            .overflow_factor(config.overflow_factor())
            .heap_size(config.heap_size())
            .segment_size(config.segment_size())
            .eviction(eviction)
            .datapool_path(config.datapool_path())
            .build();

        Self { time_type, data }
    }
}

impl Storage for SegCache {
    fn expire(&mut self) {
        self.data.expire();
    }
}

impl GetTtl for SegCache {
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
