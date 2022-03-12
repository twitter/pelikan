// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! This module defines how `Seg` storage will be used to execute `Memcache`
//! storage commands.

use super::*;

use protocol_memcache::*;

use std::time::Duration;

impl<'a> Execute<MemcacheRequest, MemcacheResponse> for Seg {
    fn execute(&mut self, request: MemcacheRequest) -> Option<MemcacheResponse> {
        let result = match request {
            MemcacheRequest::Get { ref keys } => MemcacheResult::Values {
                entries: self.get(keys),
                cas: false,
            },
            MemcacheRequest::Gets { ref keys } => MemcacheResult::Values {
                entries: self.get(keys),
                cas: true,
            },
            MemcacheRequest::Set { ref entry, noreply } => {
                let response = match self.set(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Add { ref entry, noreply } => {
                let response = match self.add(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Replace { ref entry, noreply } => {
                let response = match self.replace(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Append { ref entry, noreply } => {
                let response = match self.append(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    Err(MemcacheStorageError::NotSupported) => MemcacheResult::Error,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Prepend { ref entry, noreply } => {
                let response = match self.prepend(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    Err(MemcacheStorageError::NotSupported) => MemcacheResult::Error,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Delete { ref key, noreply } => {
                let response = match self.delete(key) {
                    Ok(_) => MemcacheResult::Deleted,
                    Err(MemcacheStorageError::NotFound) => MemcacheResult::NotFound,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Incr {
                ref key,
                value,
                noreply,
            } => {
                let response = match self.incr(key, value) {
                    Ok(count) => MemcacheResult::Count(count),
                    Err(MemcacheStorageError::NotFound) => MemcacheResult::NotFound,
                    Err(MemcacheStorageError::NotSupported) => MemcacheResult::Error,
                    Err(MemcacheStorageError::Error) => MemcacheResult::Error,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Decr {
                ref key,
                value,
                noreply,
            } => {
                let response = match self.decr(key, value) {
                    Ok(count) => MemcacheResult::Count(count),
                    Err(MemcacheStorageError::NotFound) => MemcacheResult::NotFound,
                    Err(MemcacheStorageError::NotSupported) => MemcacheResult::Error,
                    Err(MemcacheStorageError::Error) => MemcacheResult::Error,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Cas { ref entry, noreply } => {
                let response = match self.cas(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotFound) => MemcacheResult::NotFound,
                    Err(MemcacheStorageError::Exists) => MemcacheResult::Exists,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::FlushAll => {
                return None;
            }
        };

        Some(MemcacheResponse { request, result })
    }
}

impl MemcacheStorage for Seg {
    fn get(&mut self, keys: &[Box<[u8]>]) -> Box<[MemcacheEntry]> {
        let mut items = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(item) = self.data.get(key) {
                let o = item.optional().unwrap_or(&[0, 0, 0, 0]);
                let flags = u32::from_be_bytes([o[0], o[1], o[2], o[3]]);
                items.push(MemcacheEntry {
                    key: item.key().to_vec().into_boxed_slice(),
                    value: Some(item.value().to_owned()),
                    flags,
                    cas: Some(item.cas().into()),
                    ttl: None,
                });
            } else {
                items.push(MemcacheEntry {
                    key: key.to_vec().into_boxed_slice(),
                    value: None,
                    flags: 0,
                    cas: None,
                    ttl: None,
                });
            }
        }

        items.into_boxed_slice()
    }

    fn set(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = if entry.ttl().map(|v| v.as_secs()) == Some(0) {
            return Err(MemcacheStorageError::NotStored);
        } else {
            entry
                .ttl()
                .map(|v| Duration::from_secs(v.as_secs()))
                .unwrap_or_else(|| Duration::from_secs(0))
        };

        match self.data.insert(
            entry.key(),
            entry.value().unwrap_or_else(|| b"".into()),
            Some(&entry.flags().to_be_bytes()),
            ttl,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(MemcacheStorageError::NotStored),
        }
    }

    fn add(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = if entry.ttl().map(|v| v.as_secs()) == Some(0) {
            return Err(MemcacheStorageError::NotStored);
        } else {
            entry
                .ttl()
                .map(|v| Duration::from_secs(v.as_secs()))
                .unwrap_or_else(|| Duration::from_secs(0))
        };

        if self.data.get_no_freq_incr(entry.key()).is_none()
            && self
                .data
                .insert(
                    entry.key(),
                    entry.value().unwrap_or_else(|| b"".into()),
                    Some(&entry.flags().to_be_bytes()),
                    ttl,
                )
                .is_ok()
        {
            Ok(())
        } else {
            Err(MemcacheStorageError::NotStored)
        }
    }

    fn replace(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = if entry.ttl().map(|v| v.as_secs()) == Some(0) {
            return Err(MemcacheStorageError::NotStored);
        } else {
            entry
                .ttl()
                .map(|v| Duration::from_secs(v.as_secs()))
                .unwrap_or_else(|| Duration::from_secs(0))
        };

        if self.data.get_no_freq_incr(entry.key()).is_some()
            && self
                .data
                .insert(
                    entry.key(),
                    entry.value().unwrap_or_else(|| b"".into()),
                    Some(&entry.flags().to_be_bytes()),
                    ttl,
                )
                .is_ok()
        {
            Ok(())
        } else {
            Err(MemcacheStorageError::NotStored)
        }
    }

    fn append(&mut self, _entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        Err(MemcacheStorageError::NotSupported)
    }

    fn prepend(&mut self, _entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        Err(MemcacheStorageError::NotSupported)
    }

    fn delete(&mut self, key: &[u8]) -> Result<(), MemcacheStorageError> {
        if self.data.delete(key) {
            Ok(())
        } else {
            Err(MemcacheStorageError::NotFound)
        }
    }

    fn incr(&mut self, key: &[u8], value: u64) -> Result<u64, MemcacheStorageError> {
        match self.data.wrapping_add(key, value) {
            Ok(item) => match item.value() {
                Value::U64(v) => Ok(v),
                _ => Err(MemcacheStorageError::ServerError),
            },
            Err(SegError::NotFound) => Err(MemcacheStorageError::NotFound),
            Err(SegError::NotNumeric) => Err(MemcacheStorageError::Error),
            Err(_) => Err(MemcacheStorageError::ServerError),
        }
    }

    fn decr(&mut self, key: &[u8], value: u64) -> Result<u64, MemcacheStorageError> {
        match self.data.saturating_sub(key, value) {
            Ok(item) => match item.value() {
                Value::U64(v) => Ok(v),
                _ => Err(MemcacheStorageError::ServerError),
            },
            Err(SegError::NotFound) => Err(MemcacheStorageError::NotFound),
            Err(SegError::NotNumeric) => Err(MemcacheStorageError::Error),
            Err(_) => Err(MemcacheStorageError::ServerError),
        }
    }

    fn cas(&mut self, entry: &MemcacheEntry) -> Result<(), MemcacheStorageError> {
        let ttl = if entry.ttl().map(|v| v.as_secs()) == Some(0) {
            return Err(MemcacheStorageError::NotStored);
        } else {
            entry
                .ttl()
                .map(|v| Duration::from_secs(v.as_secs()))
                .unwrap_or_else(|| Duration::from_secs(0))
        };

        match self.data.cas(
            entry.key(),
            entry.value().unwrap_or_else(|| b"".into()),
            Some(&entry.flags().to_be_bytes()),
            ttl,
            entry.cas().unwrap_or(0) as u32,
        ) {
            Ok(_) => Ok(()),
            Err(SegError::NotFound) => Err(MemcacheStorageError::NotFound),
            Err(SegError::Exists) => Err(MemcacheStorageError::Exists),
            Err(_) => Err(MemcacheStorageError::NotStored),
        }
    }
}
