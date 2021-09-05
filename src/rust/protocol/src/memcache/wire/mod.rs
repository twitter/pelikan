// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;
use crate::*;

use metrics::{pelikan_metrics, Counter};

pelikan_metrics! {
    static GET: Counter;
    static GET_KEY: Counter;
    static GET_KEY_HIT: Counter;
    static GET_KEY_MISS: Counter;

    static GETS: Counter;
    static GETS_KEY: Counter;
    static GETS_KEY_HIT: Counter;
    static GETS_KEY_MISS: Counter;

    static SET: Counter;
    static SET_STORED: Counter;
    static SET_NOT_STORED: Counter;

    static ADD: Counter;
    static ADD_STORED: Counter;
    static ADD_NOT_STORED: Counter;

    static REPLACE: Counter;

    static DELETE: Counter;

    static CAS: Counter;
}

impl<'a, T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
    fn execute(&mut self, request: MemcacheRequest) -> Option<MemcacheResponse> {
        let response = match request {
            MemcacheRequest::Get { keys } => {
                GET.increment();

                let entries = self.get(&keys);

                GET_KEY.add(keys.len() as _);
                GET_KEY_HIT.add(entries.len() as _);
                GET_KEY_MISS.add(keys.len() as _);

                MemcacheResponse::Values {
                    entries,
                    cas: false,
                }
            }
            MemcacheRequest::Gets { keys } => {
                GETS.increment();

                let entries = self.get(&keys);

                GETS_KEY.add(keys.len() as _);
                GETS_KEY_HIT.add(keys.len() as _);
                GETS_KEY_MISS.add(keys.len() as _);

                MemcacheResponse::Values { entries, cas: true }
            }
            MemcacheRequest::Set { entry, noreply } => {
                SET.increment();

                let response = match self.set(entry) {
                    Ok(_) => {
                        SET_STORED.increment();
                        MemcacheResponse::Stored
                    }
                    Err(MemcacheStorageError::NotStored) => {
                        SET_NOT_STORED.increment();
                        MemcacheResponse::NotStored
                    }
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Add { entry, noreply } => {
                ADD.increment();

                let response = match self.add(entry) {
                    Ok(_) => {
                        ADD_STORED.increment();
                        MemcacheResponse::Stored
                    }
                    Err(MemcacheStorageError::NotStored) => {
                        ADD_NOT_STORED.increment();
                        MemcacheResponse::NotStored
                    }
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Replace { entry, noreply } => {
                REPLACE.increment();
                let response = match self.replace(entry) {
                    Ok(_) => MemcacheResponse::Stored,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Delete { key, noreply } => {
                DELETE.increment();
                let response = match self.delete(&key) {
                    Ok(_) => MemcacheResponse::Deleted,
                    Err(MemcacheStorageError::NotFound) => MemcacheResponse::NotFound,
                    _ => {
                        unreachable!()
                    }
                };
                if noreply {
                    return None;
                }
                response
            }
            MemcacheRequest::Cas { entry, noreply } => {
                CAS.increment();
                let response = match self.cas(entry) {
                    Ok(_) => MemcacheResponse::Stored,
                    Err(MemcacheStorageError::NotFound) => MemcacheResponse::NotFound,
                    Err(MemcacheStorageError::Exists) => MemcacheResponse::Exists,
                    Err(MemcacheStorageError::NotStored) => MemcacheResponse::NotStored,
                };
                if noreply {
                    return None;
                }
                response
            }
        };

        Some(response)
    }
}
