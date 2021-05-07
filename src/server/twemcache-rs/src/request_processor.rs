// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A wrapper struct which owns the cache data, processes requests, and writes
//! responses onto session write buffers.

use crate::protocol::data::*;
use crate::*;
use bytes::BytesMut;
use config::segcache::Eviction;
use config::TimeType;
use metrics::*;
use rustcommon_time::CoarseDuration;
use segcache::*;
use std::time::SystemTime;

/// Cache struct is used to process requests and write the responses back to
/// the session write buffer
pub struct RequestProcessor {
    config: Arc<Config>,
    data: SegCache,
}

impl RequestProcessor {
    pub(crate) fn new(config: Arc<Config>) -> Self {
        let eviction = match config.segcache().eviction() {
            Eviction::None => Policy::None,
            Eviction::Random => Policy::Random,
            Eviction::Fifo => Policy::Fifo,
            Eviction::Cte => Policy::Cte,
            Eviction::Util => Policy::Util,
            Eviction::Merge => Policy::Merge {
                max: config.segcache().merge_max(),
                merge: config.segcache().merge_target(),
                compact: config.segcache().compact_target(),
            },
        };

        let data = SegCache::builder()
            .power(config.segcache().hash_power())
            .overflow_factor(config.segcache().overflow_factor())
            .heap_size(config.segcache().heap_size())
            .segment_size(config.segcache().segment_size())
            .eviction(eviction)
            .datapool_path(config.segcache().datapool_path())
            .build();

        Self { config, data }
    }

    pub(crate) fn expire(&mut self) {
        self.data.expire();
    }

    pub(crate) fn process(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        match request.command() {
            MemcacheCommand::Get => self.process_get(request, write_buffer),
            MemcacheCommand::Gets => self.process_gets(request, write_buffer),
            MemcacheCommand::Set => self.process_set(request, write_buffer),
            MemcacheCommand::Add => self.process_add(request, write_buffer),
            MemcacheCommand::Replace => self.process_replace(request, write_buffer),
            MemcacheCommand::Cas => self.process_cas(request, write_buffer),
            MemcacheCommand::Delete => self.process_delete(request, write_buffer),
        }
    }

    fn process_get(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        let mut total = 0;
        let mut found = 0;
        increment_counter!(&Stat::Get);
        for key in request.keys() {
            if let Some(item) = self.data.get(key) {
                let response = MemcacheResponse::Item { item, cas: false };
                response.serialize(write_buffer);
                found += 1;
            }
            total += 1;
        }

        increment_counter_by!(&Stat::GetKey, total);
        increment_counter_by!(&Stat::GetKeyHit, found);
        increment_counter_by!(&Stat::GetKeyMiss, total - found);

        trace!(
            "get request processed, {} out of {} keys found",
            found,
            total
        );
        MemcacheResponse::End.serialize(write_buffer);
    }

    fn process_gets(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        let mut total = 0;
        let mut found = 0;
        increment_counter!(&Stat::Gets);
        for key in request.keys() {
            if let Some(item) = self.data.get(key) {
                let response = MemcacheResponse::Item { item, cas: true };
                response.serialize(write_buffer);
                found += 1;
            }
            total += 1;
        }

        increment_counter_by!(&Stat::GetsKey, total);
        increment_counter_by!(&Stat::GetsKeyHit, found);
        increment_counter_by!(&Stat::GetsKeyMiss, total - found);

        trace!(
            "gets request processed, {} out of {} keys found",
            found,
            total
        );
        MemcacheResponse::End.serialize(write_buffer);
    }

    fn process_set(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        increment_counter!(&Stat::Set);
        let reply = !request.noreply();
        let ttl = self.get_ttl(&request);
        let key = request.keys().next().unwrap();
        let value = request.value().unwrap_or(b"");
        match self.data.insert(
            key,
            value,
            Some(&request.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
        ) {
            Ok(_) => {
                increment_counter!(&Stat::SetStored);
                if reply {
                    MemcacheResponse::Stored.serialize(write_buffer);
                }
            }
            Err(_) => {
                increment_counter!(&Stat::SetNotstored);
                if reply {
                    MemcacheResponse::NotStored.serialize(write_buffer);
                }
            }
        }
    }

    fn process_add(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        increment_counter!(&Stat::Add);
        let reply = !request.noreply();
        let ttl = self.get_ttl(&request);
        let key = request.keys().next().unwrap();
        let value = request.value().unwrap_or(b"");
        if self.data.get_no_freq_incr(key).is_none()
            && self
                .data
                .insert(
                    key,
                    value,
                    Some(&request.flags().to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::AddStored);
            if reply {
                MemcacheResponse::Stored.serialize(write_buffer);
            }
        } else {
            increment_counter!(&Stat::AddNotstored);
            if reply {
                MemcacheResponse::NotStored.serialize(write_buffer);
            }
        }
    }

    fn process_replace(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        increment_counter!(&Stat::Replace);
        let reply = !request.noreply();
        let ttl = self.get_ttl(&request);
        let key = request.keys().next().unwrap();
        let value = request.value().unwrap_or(b"");
        if self.data.get_no_freq_incr(key).is_some()
            && self
                .data
                .insert(
                    key,
                    value,
                    Some(&request.flags().to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::ReplaceStored);
            if reply {
                MemcacheResponse::Stored.serialize(write_buffer);
            }
        } else {
            increment_counter!(&Stat::ReplaceNotstored);
            if reply {
                MemcacheResponse::NotStored.serialize(write_buffer);
            }
        }
    }

    fn process_cas(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        increment_counter!(&Stat::Cas);
        let reply = !request.noreply();
        let ttl = self.get_ttl(&request);
        let key = request.keys().next().unwrap();
        let value = request.value().unwrap_or(b"");
        match self.data.cas(
            key,
            value,
            Some(&request.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
            request.cas() as u32,
        ) {
            Ok(_) => {
                increment_counter!(&Stat::CasStored);
                if reply {
                    MemcacheResponse::Stored.serialize(write_buffer);
                }
            }
            Err(SegCacheError::NotFound) => {
                increment_counter!(&Stat::CasNotfound);
                if reply {
                    MemcacheResponse::NotFound.serialize(write_buffer);
                }
            }
            Err(SegCacheError::Exists) => {
                increment_counter!(&Stat::CasExists);
                if reply {
                    MemcacheResponse::Exists.serialize(write_buffer);
                }
            }
            Err(_) => {
                increment_counter!(&Stat::CasEx);
                if reply {
                    MemcacheResponse::NotStored.serialize(write_buffer);
                }
            }
        }
    }

    fn process_delete(&mut self, request: MemcacheRequest, write_buffer: &mut BytesMut) {
        increment_counter!(&Stat::Delete);
        let reply = !request.noreply();
        let key = request.keys().next().unwrap();
        if self.data.delete(key) {
            increment_counter!(&Stat::DeleteDeleted);
            if reply {
                MemcacheResponse::Deleted.serialize(write_buffer);
            }
        } else {
            increment_counter!(&Stat::DeleteNotfound);
            if reply {
                MemcacheResponse::NotFound.serialize(write_buffer);
            }
        }
    }

    /// converts the request expiry to a ttl
    fn get_ttl(&mut self, request: &MemcacheRequest) -> u32 {
        match self.config.time().time_type() {
            TimeType::Unix => {
                let epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as u32;
                request.expiry().wrapping_sub(epoch)
            }
            TimeType::Delta => request.expiry(),
            TimeType::Memcache => {
                if request.expiry() == 0 {
                    0
                } else if request.expiry() < 60 * 60 * 24 * 30 {
                    request.expiry()
                } else {
                    let epoch = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as u32;
                    request.expiry().wrapping_sub(epoch)
                }
            }
        }
    }
}
