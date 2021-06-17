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

pub struct TtlBuckets {
    pub(crate) buckets: Box<[TtlBucket]>,
}

impl TtlBuckets {
    pub fn new() -> Self {
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

        Self { buckets }
    }

    #[cfg(feature = "dump")]
    pub(crate) fn dump(&self) -> Vec<TtlBucketDump> {
        let mut ret = Vec::new();
        for bucket in self.buckets.iter() {
            ret.push(TtlBucketDump {
                ttl: bucket.ttl,
                head: bucket.head,
            });
        }
        ret
    }

    pub(crate) fn get_bucket_index(&self, ttl: CoarseDuration) -> usize {
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
    pub(crate) fn get_mut_bucket(&mut self, ttl: CoarseDuration) -> &mut TtlBucket {
        let index = self.get_bucket_index(ttl);

        // NOTE: since get_bucket_index() must return an index within the slice,
        // we do not need to worry about UB here.
        unsafe { self.buckets.get_unchecked_mut(index) }
    }

    pub(crate) fn expire(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
        let start = Instant::now();
        let mut expired = 0;
        for bucket in self.buckets.iter_mut() {
            expired += bucket.expire(hashtable, segments);
        }
        let duration = start.elapsed();
        debug!("expired: {} segments in {:?}", expired, duration);
        increment_counter_by!(&Stat::ExpireTime, duration.as_nanos() as u64);
        expired
    }
}

impl Default for TtlBuckets {
    fn default() -> Self {
        Self::new()
    }
}
