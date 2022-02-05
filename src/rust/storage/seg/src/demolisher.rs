// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A demolisher for gracefully deconstructing a [`Seg`] instance.

use crate::*;
use std::path::PathBuf;

/// A demolisher that is used to gracefully deconstruct a [`Seg`] instance.
pub struct Demolisher {
    heap_size: usize,
    overflow_factor: f64,
    // path at which the `Segments` fields' will be stored
    segments_fields_path: Option<PathBuf>,
    // path at which the `TtlBuckets` will be stored
    ttl_buckets_path: Option<PathBuf>,
    // path at which the `Hashtable` will be stored
    hashtable_path: Option<PathBuf>,
}

// Defines the default parameters
impl Default for Demolisher {
    fn default() -> Self {
        Self {
            heap_size: 64 * 1024 * 1024,
            overflow_factor: 0.0,
            segments_fields_path: None,
            ttl_buckets_path: None,
            hashtable_path: None,
        }
    }
}


impl Demolisher {

    /// Function the same as from `SegmentsBuilder`.
    /// Specify the total heap size in bytes. The heap size will be divided by
    /// the segment size to determine the number of segments to allocate.
    pub fn heap_size(mut self, bytes: usize) -> Self {
        self.heap_size = bytes;
        self
    }

    /// Function the same as from `Builder`.
    /// Specify an overflow factor which was used to scale the `HashTable` and
    /// provide additional capacity for chaining item buckets. A factor of 1.0
    /// will result in a hash table that is 100% larger.
    /// Used for demolishing the `HashTable`
    pub fn overflow_factor(mut self, percent: f64) -> Self {
        self.overflow_factor = percent;
        self
    }

    // Set `Segments` fields' path
    pub fn segments_fields_path(mut self, path : Option<PathBuf>) -> Self {
        self.segments_fields_path = path;
        self
    }

    // Set `TtlBuckets` path
    pub fn ttl_buckets_path(mut self, path : Option<PathBuf>) -> Self {
        self.ttl_buckets_path = path;
        self
    }

    // Set `Hashtable` path
    pub fn hashtable_path(mut self, path : Option<PathBuf>) -> Self {
        self.hashtable_path = path;
        self
    }

    // Demolish the cache by attempting to save the `Segments`,
    // `TtlBuckets` and `HashTable` to the paths specified
    // If successful, return True. Else, return False.
    pub fn demolish(self, cache : Seg) -> bool {
        cache.segments.demolish(self.segments_fields_path, 
                                self.heap_size) &&
        cache.ttl_buckets.demolish(self.ttl_buckets_path) &&
        cache.hashtable.demolish(self.hashtable_path,
                                 self.overflow_factor)
    }



}
