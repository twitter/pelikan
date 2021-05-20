// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;
use metrics::Stat;
use protocol::memcache::MemcacheEntry;
use protocol::memcache::MemcacheStorageError;
use protocol::memcache::MemcacheStorage;

impl MemcacheStorage for SegCache {
    fn get(&mut self, keys: &[Box<[u8]>]) -> Box<[MemcacheEntry]> {
        increment_counter!(&Stat::Get);
        let mut items = Vec::new();
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheEntry {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: item.value().to_vec().into_boxed_slice(),
                    flags,
                    cas: Some(item.cas().into()),
                    expiry: 0,
                });
            }
        }

        items.into_boxed_slice()
    }

    fn set(
        &mut self,
        entry: MemcacheEntry,
    ) -> Result<(), MemcacheStorageError> {
        increment_counter!(&Stat::Set);
        let ttl = self.get_ttl(entry.expiry());
        match self.data.insert(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
        ) {
            Ok(_) => {
                Ok(())
            }
            Err(_) => {
                Err(MemcacheStorageError::NotStored)
            }
        }
    }

    fn add(
        &mut self,
        entry: MemcacheEntry,
    ) -> Result<(), MemcacheStorageError> {
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
            Ok(())
        } else {
            
            Err(MemcacheStorageError::NotStored)
        }
    }

    fn replace(
        &mut self,
        entry: MemcacheEntry,
    ) -> Result<(), MemcacheStorageError> {
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
            Ok(())
        } else {
            Err(MemcacheStorageError::NotStored)
        }
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MemcacheStorageError> {
        increment_counter!(&Stat::Delete);
        if self.data.delete(key) {
            Ok(())
        } else {
            Err(MemcacheStorageError::NotFound)
        }
    }

    fn cas(
        &mut self,
        entry: MemcacheEntry,
    ) -> Result<(), MemcacheStorageError> {
        increment_counter!(&Stat::Cas);
        let ttl = self.get_ttl(entry.expiry());
        match self.data.cas(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
            entry.cas().unwrap_or(0) as u32,
        ) {
            Ok(_) => {
                Ok(())
            }
            Err(SegCacheError::NotFound) => {
                Err(MemcacheStorageError::NotFound)
            }
            Err(SegCacheError::Exists) => {
                Err(MemcacheStorageError::Exists)
            }
            Err(_) => {
                Err(MemcacheStorageError::NotStored)
            }
        }
    }
}
