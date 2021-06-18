// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Possible errors returned by segment operations.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum SegmentsError {
    #[error("bad segment id")]
    BadSegmentId,
    #[error("item relink failure")]
    RelinkFailure,
    #[error("no evictable segments")]
    NoEvictableSegments,
    #[error("evict failure")]
    EvictFailure,
}
