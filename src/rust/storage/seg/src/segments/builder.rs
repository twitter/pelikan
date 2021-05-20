// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A builder struct for initializing segment storage.

use crate::eviction::*;
use crate::item::*;
use crate::segments::*;

use std::path::{Path, PathBuf};

/// The `SegmentsBuilder` allows for the configuration of the segment storage.
pub(crate) struct SegmentsBuilder {
    pub(super) heap_size: usize,
    pub(super) segment_size: i32,
    pub(super) evict_policy: Policy,
    pub(super) datapool_path: Option<PathBuf>,
}

impl Default for SegmentsBuilder {
    fn default() -> Self {
        Self {
            segment_size: 1024 * 1024,
            heap_size: 64 * 1024 * 1024,
            evict_policy: Policy::Random,
            datapool_path: None,
        }
    }
}

impl<'a> SegmentsBuilder {
    /// Set the segment size in bytes.
    ///
    /// # Panics
    ///
    /// This function will panic if the size is not greater than the per-item
    /// overhead. Currently this means that the minimum size is 6 bytes when
    /// built without magic/debug, or 10 bytes when built with magic/debug.
    pub fn segment_size(mut self, bytes: i32) -> Self {
        #[cfg(not(feature = "magic"))]
        assert!(bytes > ITEM_HDR_SIZE as i32);

        #[cfg(feature = "magic")]
        assert!(bytes > ITEM_HDR_SIZE as i32 + ITEM_MAGIC_SIZE as i32);

        self.segment_size = bytes;
        self
    }

    /// Specify the total heap size in bytes. The heap size will be divided by
    /// the segment size to determine the number of segments to allocate.
    pub fn heap_size(mut self, bytes: usize) -> Self {
        self.heap_size = bytes;
        self
    }

    /// Specify the eviction [`Policy`] which will be used when item allocation
    /// fails due to memory pressure.
    pub fn eviction_policy(mut self, policy: Policy) -> Self {
        self.evict_policy = policy;
        self
    }

    /// Specify a backing file to be used for the segment storage. If provided,
    /// a file will be created at the corresponding path and used for segment
    /// storage.
    pub fn datapool_path<T: AsRef<Path>>(mut self, path: Option<T>) -> Self {
        self.datapool_path = path.map(|p| p.as_ref().to_owned());
        self
    }

    /// Construct the [`Segments`] from the builder
    pub fn build(self) -> Segments {
        Segments::from_builder(self)
    }
}
