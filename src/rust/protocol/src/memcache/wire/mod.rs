// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;
use crate::*;

impl<'a, T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
    fn execute(&mut self, request: MemcacheRequest) -> Option<MemcacheResponse> {
        let response = match request {
            MemcacheRequest::Get { keys } => MemcacheResponse::Values { entries: self.get(&keys), cas: false },
            MemcacheRequest::Gets { keys } => MemcacheResponse::Values { entries: self.get(&keys), cas: true },
            MemcacheRequest::Set { entry, noreply } => {
                let result = self.set(entry);
                if noreply {
                    return None;
                }
                match result {
                    Ok(_) => MemcacheResponse::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                    _ => { unreachable!() },
                }
            }
            MemcacheRequest::Add { entry, noreply } => {
                let result = self.add(entry);
                if noreply {
                    return None;
                }
                match result {
                    Ok(_) => MemcacheResponse::Stored,
                    Err(MemcacheStorageError::Exists) => MemcacheResponse::Exists,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                    _ => { unreachable!() },
                }
            }
            MemcacheRequest::Replace { entry, noreply } => {
                let result = self.replace(entry);
                if noreply {
                    return None;
                }
                match result {
                    Ok(_) => MemcacheResponse::Stored,
                    Err(MemcacheStorageError::NotFound) => MemcacheResponse::NotFound,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                    _ => { unreachable!() },
                }
            }
            MemcacheRequest::Delete { key, noreply } => {
                let result = self.delete(&key);
                if noreply {
                    return None;
                }
                match result {
                    Ok(_) => MemcacheResponse::Deleted,
                    Err(MemcacheStorageError::NotFound) => MemcacheResponse::NotFound,
                    _ => { unreachable!() },
                }
            }
            MemcacheRequest::Cas { entry, noreply } => {
                let result = self.cas(entry);
                if noreply {
                    return None;
                }
                match result {
                    Ok(_) => MemcacheResponse::Deleted,
                    Err(MemcacheStorageError::NotFound) => MemcacheResponse::NotFound,
                    Err(MemcacheStorageError::Exists) => MemcacheResponse::Exists,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                }
            }
        };
        
        Some(response)
    }
}
