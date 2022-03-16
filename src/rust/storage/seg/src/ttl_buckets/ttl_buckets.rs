// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A collection of [`TtlBucket`]s which covers the full range of TTLs.
//!
//! We use a total of 1024 buckets to represent the full range of TTLs. We
//! divide the buckets into 4 ranges:
//! * 1-2048s (1 second - ~34 minutes) are stored in buckets which are 8s wide.
//! * 2048-32_768s (~34 minutes - ~9 hours) are stored in buckets which are 128s
//!   (~2 minutes) wide.
//! * 32_768-524_288s (~9 hours - ~6 days) are stored in buckets which are 2048s
//!   (~34 minutes) wide.
//! * 524_288-8_388_608s (~6 days - ~97 days) are stored in buckets which are
//!   32_768s (~9 hours) wide.
//! * TTLs beyond 8_388_608s (~97 days) and TTLs of 0 are all treated as the max
//!   TTL.
//!
//! See the
//! [Segcache paper](https://www.usenix.org/system/files/nsdi21-yang.pdf) for
//! more detail.

use super::{CLEAR_TIME, EXPIRE_TIME};
use crate::*;

const N_BUCKET_PER_STEP_N_BIT: usize = 8;
const N_BUCKET_PER_STEP: usize = 1 << N_BUCKET_PER_STEP_N_BIT;

const TTL_BUCKET_INTERVAL_N_BIT_1: usize = 3;
const TTL_BUCKET_INTERVAL_N_BIT_2: usize = 7;
const TTL_BUCKET_INTERVAL_N_BIT_3: usize = 11;
const TTL_BUCKET_INTERVAL_N_BIT_4: usize = 15;

const TTL_BUCKET_INTERVAL_1: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_1;
const TTL_BUCKET_INTERVAL_2: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_2;
const TTL_BUCKET_INTERVAL_3: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_3;
const TTL_BUCKET_INTERVAL_4: usize = 1 << TTL_BUCKET_INTERVAL_N_BIT_4;

const TTL_BOUNDARY_1: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_1 + N_BUCKET_PER_STEP_N_BIT);
const TTL_BOUNDARY_2: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_2 + N_BUCKET_PER_STEP_N_BIT);
const TTL_BOUNDARY_3: i32 = 1 << (TTL_BUCKET_INTERVAL_N_BIT_3 + N_BUCKET_PER_STEP_N_BIT);

const MAX_N_TTL_BUCKET: usize = N_BUCKET_PER_STEP * 4;
const MAX_TTL_BUCKET_IDX: usize = MAX_N_TTL_BUCKET - 1;
#[derive(Clone)]
pub struct TtlBuckets {
    pub(crate) buckets: Box<[TtlBucket]>,
    pub(crate) last_expired: Instant,
    /// Are `TtlBuckets` copied back from a file?
    pub(crate) _buckets_copied_back: bool,
}

impl TtlBuckets {
    /// Create a new set of `TtlBuckets` which cover the full range of TTLs. See
    /// the module-level documentation for how the range of TTLs are stored.
    pub fn new() -> Self {
        // TODO: add path as argument
        let intervals = [
            TTL_BUCKET_INTERVAL_1,
            TTL_BUCKET_INTERVAL_2,
            TTL_BUCKET_INTERVAL_3,
            TTL_BUCKET_INTERVAL_4,
        ];

        let mut buckets = Vec::with_capacity(0);
        buckets.reserve_exact(intervals.len() * N_BUCKET_PER_STEP as usize);

        for interval in &intervals {
            for j in 0..N_BUCKET_PER_STEP {
                let ttl = interval * j + 1;
                let bucket = TtlBucket::new(ttl as i32);
                buckets.push(bucket);
            }
        }

        let buckets = buckets.into_boxed_slice();
        let last_expired = Instant::now();

        Self {
            buckets,
            last_expired,
            _buckets_copied_back: false,
        }
    }

    // Returns a restored `TtlBuckets` using recovery data (`metadata`)
    pub fn restore(metadata: &[u8]) -> Self {
        let bucket_size = ::std::mem::size_of::<TtlBucket>();
        let last_expired_size = ::std::mem::size_of::<Instant>();
        let ttl_buckets_struct_size = MAX_N_TTL_BUCKET * bucket_size // `buckets` 
                                        + last_expired_size;

        // create blank bytes to copy data into
        let mut bytes = vec![0; ttl_buckets_struct_size];
        // retrieve bytes from mmapped file
        bytes.copy_from_slice(&metadata[0..ttl_buckets_struct_size]);

        let mut offset = 0;
        // ----- Retrieve `last_expired` -----
        let mut end = last_expired_size;
        let last_expired =
            unsafe { *(bytes[offset..last_expired_size].as_mut_ptr() as *mut Instant) };

        offset += last_expired_size;
        // ----- Retrieve `buckets` -----

        let mut buckets = Vec::with_capacity(0);
        buckets.reserve_exact(MAX_N_TTL_BUCKET);

        // Get each `TtlBucket` from the raw bytes
        for _ in 0..MAX_N_TTL_BUCKET {
            end += bucket_size;

            // cast bytes to `TtlBucket`
            let bucket = unsafe { *(bytes[offset..end].as_mut_ptr() as *mut TtlBucket) };
            buckets.push(bucket);

            offset += bucket_size;
        }

        let buckets = buckets.into_boxed_slice();

        Self {
            buckets,
            last_expired,
            _buckets_copied_back: true,
        }
    }

    /// Flushes the `TtlBuckets` by copying it to `metadata`
    pub fn flush(&self, metadata: &mut [u8]) {
        let bucket_size = ::std::mem::size_of::<TtlBucket>();
        let last_expired_size = ::std::mem::size_of::<Instant>();

        let mut offset = 0;
        // --------------------- Store `last_expired` -----------------

        // cast `last_expired` to byte pointer
        let byte_ptr = (&self.last_expired as *const Instant) as *const u8;

        // store `last_expired` back to mmapped file
        offset =
            store::store_bytes_and_update_offset(byte_ptr, offset, last_expired_size, metadata);

        // --------------------- Store `buckets` -----------------

        // for every `TtlBucket`
        for id in 0..MAX_N_TTL_BUCKET {
            // cast `TtlBucket` to byte pointer
            let byte_ptr = (&self.buckets[id] as *const TtlBucket) as *const u8;

            // store `TtlBucket` back to mmapped file
            offset = store::store_bytes_and_update_offset(byte_ptr, offset, bucket_size, metadata);
        }

        // --------------------------------------------------
    }

    pub(crate) fn get_bucket_index(&self, ttl: Duration) -> usize {
        let ttl = ttl.as_secs() as i32;
        if ttl <= 0 {
            self.buckets.len() - 1
        } else if ttl & !(TTL_BOUNDARY_1 - 1) == 0 {
            (ttl >> TTL_BUCKET_INTERVAL_N_BIT_1) as usize
        } else if ttl & !(TTL_BOUNDARY_2 - 1) == 0 {
            (ttl >> TTL_BUCKET_INTERVAL_N_BIT_2) as usize + N_BUCKET_PER_STEP
        } else if ttl & !(TTL_BOUNDARY_3 - 1) == 0 {
            (ttl >> TTL_BUCKET_INTERVAL_N_BIT_3) as usize + N_BUCKET_PER_STEP * 2
        } else {
            let bucket_idx = (ttl >> TTL_BUCKET_INTERVAL_N_BIT_4) as usize + N_BUCKET_PER_STEP * 3;
            if bucket_idx > MAX_TTL_BUCKET_IDX {
                MAX_TTL_BUCKET_IDX
            } else {
                bucket_idx
            }
        }
    }

    // TODO(bmartin): confirm handling for negative TTLs here...
    /// Get a mutable reference to the `TtlBucket` for the given TTL.
    pub(crate) fn get_mut_bucket(&mut self, ttl: Duration) -> &mut TtlBucket {
        let index = self.get_bucket_index(ttl);

        // NOTE: since get_bucket_index() must return an index within the slice,
        // we do not need to worry about UB here.
        unsafe { self.buckets.get_unchecked_mut(index) }
    }

    pub(crate) fn expire(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
        let now = Instant::now();

        if now == self.last_expired {
            return 0;
        } else {
            self.last_expired = now;
        }

        let start = Instant::now();
        let mut expired = 0;
        for bucket in self.buckets.iter_mut() {
            expired += bucket.expire(hashtable, segments);
        }
        let duration = start.elapsed();
        debug!("expired: {} segments in {:?}", expired, duration);
        EXPIRE_TIME.add(duration.as_nanos() as _);
        expired
    }

    pub(crate) fn clear(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
        let start = Instant::now();
        let mut cleared = 0;
        for bucket in self.buckets.iter_mut() {
            cleared += bucket.clear(hashtable, segments);
        }
        segments.set_flush_at(Instant::now());
        let duration = start.elapsed();
        debug!("expired: {} segments in {:?}", cleared, duration);
        CLEAR_TIME.add(duration.as_nanos() as _);
        cleared
    }

    /// TODO: this code is repeated in restore() and flush(), can it be reduced?
    /// Function used by `Builder` to calculate the number of bytes of the `TtlBuckets`
    /// that are stored/restored
    pub fn recover_size(&self) -> usize {
        let bucket_size = ::std::mem::size_of::<TtlBucket>();
        let last_expired_size = ::std::mem::size_of::<Instant>();
        MAX_N_TTL_BUCKET * bucket_size // `buckets` 
        + last_expired_size
    }
}

impl Default for TtlBuckets {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for TtlBuckets {
    // Checks if `TtlBuckets` are equivalent
    fn eq(&self, other: &Self) -> bool {
        self.buckets == other.buckets && self.last_expired == other.last_expired
    }
}
