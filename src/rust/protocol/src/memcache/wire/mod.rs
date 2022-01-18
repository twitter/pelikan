// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod request;
mod response;

pub use request::*;
pub use response::*;

use super::*;
use crate::*;

use common::time::{Duration, Instant};
use metrics::{static_metrics, Counter, Heatmap, Relaxed};

static_metrics! {
    static GET: Counter;
    static GET_EX: Counter;
    static GET_CARDINALITY: Relaxed<Heatmap> = Relaxed::new(||
        Heatmap::new(request::MAX_BATCH_SIZE as _, 3, Duration::from_secs(60), Duration::from_secs(1))
    );
    static GET_KEY: Counter;
    static GET_KEY_HIT: Counter;
    static GET_KEY_MISS: Counter;

    static GETS: Counter;
    static GETS_EX: Counter;
    static GETS_KEY: Counter;
    static GETS_KEY_HIT: Counter;
    static GETS_KEY_MISS: Counter;

    static SET: Counter;
    static SET_EX: Counter;
    static SET_STORED: Counter;
    static SET_NOT_STORED: Counter;

    static ADD: Counter;
    static ADD_EX: Counter;
    static ADD_STORED: Counter;
    static ADD_NOT_STORED: Counter;

    static REPLACE: Counter;
    static REPLACE_EX: Counter;
    static REPLACE_STORED: Counter;
    static REPLACE_NOT_STORED: Counter;

    static APPEND: Counter;
    static APPEND_EX: Counter;
    static APPEND_STORED: Counter;
    static APPEND_NOT_STORED: Counter;

    static PREPEND: Counter;
    static PREPEND_EX: Counter;
    static PREPEND_STORED: Counter;
    static PREPEND_NOT_STORED: Counter;

    static DELETE: Counter;
    static DELETE_EX: Counter;
    static DELETE_DELETED: Counter;
    static DELETE_NOT_FOUND: Counter;

    static INCR: Counter;
    static INCR_EX: Counter;
    static INCR_NOT_FOUND: Counter;

    static DECR: Counter;
    static DECR_EX: Counter;
    static DECR_NOT_FOUND: Counter;

    static CAS: Counter;
    static CAS_EX: Counter;
    static CAS_EXISTS: Counter;
    static CAS_NOT_FOUND: Counter;
    static CAS_STORED: Counter;
}

impl<'a, T> Execute<MemcacheRequest, MemcacheResponse> for T
where
    T: MemcacheStorage,
{
    fn execute(&mut self, request: MemcacheRequest) -> Option<MemcacheResponse> {
        let result = match request {
            MemcacheRequest::Get { ref keys } => {
                GET_CARDINALITY.increment(Instant::now(), keys.len() as _, 1);
                MemcacheResult::Values {
                    entries: self.get(keys),
                    cas: false,
                }
            }
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
