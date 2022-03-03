// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A builder for configuring a new [`Seg`] instance.

use crate::datapool::*;
use crate::*;
use std::path::Path;
use std::path::PathBuf;

/// A builder that is used to construct a new [`Seg`] instance.
pub struct Builder {
    restore: bool,
    hash_power: u8,
    overflow_factor: f64,
    segments_builder: SegmentsBuilder,
    metadata_path: Option<PathBuf>,
    graceful_shutdown: bool,
}

// Defines the default parameters
impl Default for Builder {
    fn default() -> Self {
        Self {
            restore: false,
            hash_power: 16,
            overflow_factor: 0.0,
            segments_builder: SegmentsBuilder::default(),
            metadata_path: None,
            graceful_shutdown: false,
        }
    }
}

impl Builder {
    /// Specify to `Builder` and `SegmentsBuilder` whether the cache will be restored.
    /// Otherwise, the cache will be created and treated as new.
    pub fn restore(mut self, restore: bool) -> Self {
        self.restore = restore;
        self.segments_builder = self.segments_builder.restore(restore);
        self
    }

    /// Specify the hash power, which limits the size of the hashtable to 2^N
    /// entries. 1/8th of these are used for metadata storage, meaning that the
    /// total number of items which can be held in the cache is limited to
    /// `7 * 2^(N - 3)` items. The hash table will have a total size of
    /// `2^(N + 3)` bytes.
    ///
    /// ```
    /// use seg::Seg;
    ///
    /// // create a cache with a small hashtable that has room for ~114k items
    /// // without using any overflow buckets.
    /// let cache = Seg::builder().hash_power(17).build();
    ///
    /// // create a cache with a larger hashtable with room for ~1.8M items
    /// let cache = Seg::builder().hash_power(21).build();
    /// ```
    pub fn hash_power(mut self, hash_power: u8) -> Self {
        assert!(hash_power >= 3, "hash power must be at least 3");
        self.hash_power = hash_power;
        self
    }

    /// Specify an overflow factor which is used to scale the hashtable and
    /// provide additional capacity for chaining item buckets. A factor of 1.0
    /// will result in a hash table that is 100% larger.
    ///
    /// ```
    /// use seg::Seg;
    ///
    /// // create a cache with a hashtable with room for ~228k items, which is
    /// // about the same as using a hash power of 18, but is more tolerant of
    /// // hash collisions.
    /// let cache = Seg::builder()
    ///     .hash_power(17)
    ///     .overflow_factor(1.0)
    ///     .build();
    ///
    /// // smaller overflow factors may be specified, meaning only some buckets
    /// // can ever be chained
    /// let cache = Seg::builder()
    ///     .hash_power(17)
    ///     .overflow_factor(0.2)
    ///     .build();
    /// ```
    pub fn overflow_factor(mut self, percent: f64) -> Self {
        self.overflow_factor = percent;
        self
    }

    /// Specify the total number of bytes to be used for heap storage of items.
    /// This includes, key, value, and per-item overheads.
    ///
    /// ```
    /// use seg::Seg;
    ///
    /// const MB: usize = 1024 * 1024;
    ///
    /// // create a cache with a 64MB heap
    /// let cache = Seg::builder().heap_size(64 * MB).build();
    ///
    /// // create a cache with a 256MB heap
    /// let cache = Seg::builder().heap_size(256 * MB).build();
    /// ```
    pub fn heap_size(mut self, bytes: usize) -> Self {
        self.segments_builder = self.segments_builder.heap_size(bytes);
        self
    }

    /// Specify the segment size for item storage. The largest item which can be
    /// held is `size - 5` bytes for builds without the `debug` or `magic` build
    /// features enabled. Smaller segment sizes reduce the number of items which
    /// would be evicted/expired at one time, at the cost of additional memory
    /// and book-keeping overheads compared to using larger segments for the
    /// same total size.
    ///
    /// ```
    /// use seg::Seg;
    ///
    /// const MB: i32 = 1024 * 1024;
    ///
    /// // create a cache using 1MB segments
    /// let cache = Seg::builder().segment_size(1 * MB).build();
    ///
    /// // create a cache using 4MB segments
    /// let cache = Seg::builder().segment_size(4 * MB).build();
    /// ```
    pub fn segment_size(mut self, size: i32) -> Self {
        self.segments_builder = self.segments_builder.segment_size(size);
        self
    }

    /// Specify the eviction policy to be used. See the `Policy` documentation
    /// for more details about each strategy.
    ///
    /// ```
    /// use seg::{Policy, Seg};
    ///
    /// // create a cache using random segment eviction
    /// let cache = Seg::builder().eviction(Policy::Random).build();
    ///
    /// // create a cache using a merge based eviction policy
    /// let policy = Policy::Merge { max: 8, merge: 4, compact: 2};
    /// let cache = Seg::builder().eviction(policy).build();
    /// ```
    pub fn eviction(mut self, policy: Policy) -> Self {
        self.segments_builder = self.segments_builder.eviction_policy(policy);
        self
    }

    /// Specify a backing file to be used for `Segments.data` storage.
    pub fn datapool_path<T: AsRef<Path>>(mut self, path: Option<T>) -> Self {
        self.segments_builder = self.segments_builder.datapool_path(path);
        self
    }

    /// Specify a backing file to be used for metadata storage.
    pub fn metadata_path<T: AsRef<Path>>(mut self, path: Option<T>) -> Self {
        self.metadata_path = path.map(|p| p.as_ref().to_owned());
        self
    }

    /// Specify whether the cache will be gracefully shutdown. If `true`, then
    /// when the cache is flushed, the relevant parts will be stored to the file
    /// with path `metadata_path`
    pub fn graceful_shutdown(mut self, graceful_shutdown: bool) -> Self {
        self.graceful_shutdown = graceful_shutdown;
        self
    }

    /// Consumes the builder and returns a fully-allocated `Seg` instance.
    /// If `restore`, the cache `Segments.data` is file backed by an existing
    /// file and a valid file for the `metadata` is given, `Seg` will be
    /// restored. Otherwise, create a new `Seg` instance. The path for the
    /// `metadata` file will be saved with the `Seg` instance to be used to save
    // the structures to upon graceful shutdown.
    ///
    /// ```
    /// use seg::{Policy, Seg};
    ///
    /// const MB: usize = 1024 * 1024;
    ///
    /// let cache = Seg::builder()
    ///     .heap_size(64 * MB)
    ///     .segment_size(1 * MB as i32)
    ///     .hash_power(16)
    ///     .eviction(Policy::Random).build();
    /// ```
    pub fn build(self) -> Seg {
        // If `restore` and there is a path for the metadata file to
        // restore from, restore the cache
        if self.restore && self.metadata_path.is_some() {
            // Check if the metadata file exists and with what size
            if let Ok(file_size) =
                std::fs::metadata(self.metadata_path.as_ref().unwrap()).map(|m| m.len())
            {
                // TODO: implement a non-messy way to calculate expected file size, rather than just taking actual size
                let file_size = file_size as usize;

                // Mmap file
                let mut pool = File::create(self.metadata_path.clone().unwrap(), file_size, true)
                    .expect("failed to allocate file backed storage");
                let metadata = pool.as_mut_slice();

                let hashtable = HashTable::restore(metadata, self.hash_power, self.overflow_factor);

                let mut offset = hashtable.recover_size();
                let ttl_buckets = TtlBuckets::restore(&metadata[offset..]);

                offset += ttl_buckets.recover_size();

                let segments = self
                    .segments_builder
                    .clone()
                    .build(Some(&mut metadata[offset..]));

                // Check that `Segments` was copied back, it will fail if the
                // file for the file backed `Segments.data` did not exist
                if segments.fields_copied_back {
                    return Seg {
                        hashtable,
                        segments,
                        ttl_buckets,
                        metadata_path: self.metadata_path,
                        graceful_shutdown: self.graceful_shutdown,
                    };
                }
            }
        }

        // Otherwise, create a new cache
        let segments = self.segments_builder.build(None);
        let hashtable = HashTable::new(self.hash_power, self.overflow_factor);
        let ttl_buckets = TtlBuckets::new();

        Seg {
            hashtable,
            segments,
            ttl_buckets,
            metadata_path: self.metadata_path,
            graceful_shutdown: self.graceful_shutdown,
        }
    }
}
