// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::*;

use protocol::memcache::{MemcacheEntry, MemcacheStorage, MemcacheStorageError};

impl MemcacheStorage for Seg {
    fn get(&mut self, keys: &[Box<[u8]>]) -> Box<[MemcacheEntry]> {
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

    fn set(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = self
            .get_ttl(entry.expiry())
            .ok_or(MemcacheStorageError::NotStored)?;
        match self.data.insert(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(MemcacheStorageError::NotStored),
        }
    }

    fn add(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = self
            .get_ttl(entry.expiry())
            .ok_or(MemcacheStorageError::NotStored)?;
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

    fn replace(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = self
            .get_ttl(entry.expiry())
            .ok_or(MemcacheStorageError::NotStored)?;
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
        if self.data.delete(key) {
            Ok(())
        } else {
            Err(MemcacheStorageError::NotFound)
        }
    }

    fn cas(&mut self, entry: MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = self
            .get_ttl(entry.expiry())
            .ok_or(MemcacheStorageError::NotStored)?;
        match self.data.cas(
            entry.key(),
            entry.value(),
            Some(&entry.flags().to_be_bytes()),
            CoarseDuration::from_secs(ttl),
            entry.cas().unwrap_or(0) as u32,
        ) {
            Ok(_) => Ok(()),
            Err(SegError::NotFound) => Err(MemcacheStorageError::NotFound),
            Err(SegError::Exists) => Err(MemcacheStorageError::Exists),
            Err(_) => Err(MemcacheStorageError::NotStored),
        }
    }
}
