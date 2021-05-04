// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Top-level errors that will be returned to a caller of this library.

use thiserror::Error;

#[derive(Error, Debug)]
/// Possible errors returned by the top-level API
pub enum SegCacheError<'a> {
    #[error("hashtable insert exception")]
    HashTableInsertEx,
    #[error("eviction exception")]
    EvictionEx,
    #[error("item oversized ({size:?} bytes) for key: {key:?}")]
    ItemOversized { size: usize, key: &'a [u8] },
    #[error("no free segments")]
    NoFreeSegments,
    #[error("item exists")]
    Exists,
    #[error("item not found")]
    NotFound,
    #[error("data corruption detected")]
    DataCorrupted,
}
