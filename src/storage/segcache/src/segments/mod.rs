// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::rand::*;
use crate::*;
use core::fmt::Debug;

use crate::common::ThinOption;

mod eviction;
mod header;
mod segment;
#[allow(clippy::module_inception)]
mod segments;

pub use eviction::{Eviction, Policy};
pub use header::{SegmentHeader, SEG_HDR_SIZE};
pub use segment::Segment;
pub(crate) use segment::SegmentDump;
pub use segments::{Segments, SegmentsBuilder, SegmentsError};

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn free_q() {
        let mut segments = Segments::builder().heap_size(16 * 1024 * 1024).build();
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
