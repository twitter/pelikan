// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segments are the backing storage of the cache.

use crate::*;

const SEG_MAGIC: u64 = 0xBADC0FFEEBADCAFE;

mod builder;
mod error;
mod header;
mod segment;
#[allow(clippy::module_inception)]
mod segments;

pub(crate) use builder::SegmentsBuilder;
pub(crate) use error::SegmentsError;
pub(crate) use header::SegmentHeader;
pub(crate) use segment::Segment;
pub(crate) use segments::Segments;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn free_q() {
        let mut segments = SegmentsBuilder::default()
            .heap_size(16 * 1024 * 1024)
            .build()
            .expect("failed to create segments");
        let mut used = Vec::new();
        for _i in 0..16 {
            let id = segments.pop_free().unwrap();
            used.push(id);
            segments.print_headers();
        }
        for id in &used {
            segments.push_free(*id);
            segments.print_headers();
        }
        for _i in 0..16 {
            let id = segments.pop_free().unwrap();
            used.push(id);
            segments.print_headers();
        }
    }
}
