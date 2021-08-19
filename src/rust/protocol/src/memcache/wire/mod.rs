// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;
use crate::*;

use metrics::Stat;

impl<'a, T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
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
            MemcacheRequest::Cas { ref entry, noreply } => {
                let response = match self.cas(entry) {
                    Ok(_) => MemcacheResult::Stored,
                    Err(MemcacheStorageError::NotFound) => MemcacheResult::NotFound,
                    Err(MemcacheStorageError::Exists) => MemcacheResult::Exists,
                    Err(MemcacheStorageError::NotStored) => MemcacheResult::NotStored,
                };
                if noreply {
                    return None;
                }
                response
            }
        };

        Some(MemcacheResponse { request, result })
    }
}
