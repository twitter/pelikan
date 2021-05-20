// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use metrics::Stat;
use protocol::memcache::MemcacheEntry;
use protocol::memcache::data::{MemcacheItem, MemcacheResponse};
use protocol::memcache::MemcacheStorage;

// move noreply into the wire protocol
// return a storage status


impl MemcacheStorage for SegCache {
    fn get(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse {
        increment_counter!(&Stat::Get);
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheItem {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: item.value().to_vec().into_boxed_slice(),
                    flags,
                    cas: None,
                });
            }
        }

        increment_counter_by!(&Stat::GetKey, keys.len() as u64);
        increment_counter_by!(&Stat::GetKeyHit, items.len() as u64);
        increment_counter_by!(&Stat::GetKeyMiss, keys.len() as u64 - items.len() as u64);

        MemcacheResponse::Items(items.into_boxed_slice())
    }

    fn gets(&mut self, keys: &[Box<[u8]>]) -> MemcacheResponse {
        increment_counter!(&Stat::Get);
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheItem {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: item.value().to_vec().into_boxed_slice(),
                    flags,
                    cas: Some(item.cas()),
                });
            }
        }

        increment_counter_by!(&Stat::GetKey, keys.len() as u64);
        increment_counter_by!(&Stat::GetKeyHit, items.len() as u64);
        increment_counter_by!(&Stat::GetKeyMiss, keys.len() as u64 - items.len() as u64);

        MemcacheResponse::Items(items.into_boxed_slice())
    }

    fn set(
        &mut self,
        entry: MemcacheEntry,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Set);
        let ttl = self.get_ttl(entry.expiry());
        match self.data.insert(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
        ) {
            Ok(_) => {
                increment_counter!(&Stat::SetStored);
                MemcacheResponse::Stored
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::Stored
                // }
            }
            Err(_) => {
                increment_counter!(&Stat::SetNotstored);
                MemcacheResponse::NotStored
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::NotStored
                // }
            }
        }
    }

    fn add(
        &mut self,
        entry: MemcacheEntry,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Add);
        let ttl = self.get_ttl(entry.expiry());
        if self.data.get_no_freq_incr(entry.key()).is_none()
            && self
                .data
                .insert(
                    entry.key(),
                    entry.value(),
                    Some(&entry.flags().to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::AddStored);
            MemcacheResponse::Stored
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::Stored
            // }
        } else {
            increment_counter!(&Stat::AddNotstored);
            MemcacheResponse::NotStored
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::NotStored
            // }
        }
    }

    fn replace(
        &mut self,
        entry: MemcacheEntry,
        // key: &[u8],
        // value: Option<Box<[u8]>>,
        // flags: u32,
        // expiry: u32,
        // noreply: bool,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Replace);
        let ttl = self.get_ttl(entry.expiry());
        if self.data.get_no_freq_incr(entry.key()).is_some()
            && self
                .data
                .insert(
                    entry.key(),
                    entry.value(),
                    Some(&entry.expiry().to_be_bytes()),
                    CoarseDuration::from_secs(ttl),
                )
                .is_ok()
        {
            increment_counter!(&Stat::ReplaceStored);
            MemcacheResponse::Stored
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::Stored
            // }
        } else {
            increment_counter!(&Stat::ReplaceNotstored);
            MemcacheResponse::NotStored
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::NotStored
            // }
        }
    }

    fn delete(&mut self, key: &[u8]) -> MemcacheResponse {
        increment_counter!(&Stat::Delete);
        if self.data.delete(key) {
            increment_counter!(&Stat::DeleteDeleted);
            MemcacheResponse::Deleted
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::Deleted
            // }
        } else {
            increment_counter!(&Stat::DeleteNotfound);
            MemcacheResponse::NotStored
            // if noreply {
            //     MemcacheResponse::NoReply
            // } else {
            //     MemcacheResponse::NotFound
            // }
        }
    }

    fn cas(
        &mut self,
        entry: MemcacheEntry,
        // key: &[u8],
        // value: Option<Box<[u8]>>,
        // flags: u32,
        // expiry: u32,
        // noreply: bool,
        // cas: u64,
    ) -> MemcacheResponse {
        increment_counter!(&Stat::Cas);
        let ttl = self.get_ttl(entry.expiry());
        match self.data.cas(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
            entry.cas() as u32,
        ) {
            Ok(_) => {
                increment_counter!(&Stat::CasStored);
                MemcacheResponse::Stored
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::Stored
                // }
            }
            Err(SegCacheError::NotFound) => {
                increment_counter!(&Stat::CasNotfound);
                MemcacheResponse::NotFound
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::NotFound
                // }
            }
            Err(SegCacheError::Exists) => {
                increment_counter!(&Stat::CasExists);
                MemcacheResponse::Exists
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::Exists
                // }
            }
            Err(_) => {
                increment_counter!(&Stat::CasEx);
                MemcacheResponse::NotStored
                // if noreply {
                //     MemcacheResponse::NoReply
                // } else {
                //     MemcacheResponse::NotStored
                // }
            }
        }
    }
}
