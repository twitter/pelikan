// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::common::ThinOption;
use crate::*;

use rustcommon_time::CoarseInstant as Instant;
use serde::{Deserialize, Serialize};

mod constants;
mod error;

pub use error::Error;

use constants::*;

pub struct TtlBucket {
    head: i32,
    tail: i32,
    ttl: i32,
    next_expiration: i32,
    segments: i32,
    next_to_merge: i32,
    last_cutoff_freq: i32,
    _pad: [u8; 36],
}

pub struct TtlBuckets {
    pub(crate) buckets: Box<[TtlBucket]>,
}

#[derive(Serialize, Deserialize)]
pub struct TtlBucketDump {
    ttl: i32,
    head: i32,
}

impl TtlBucket {
    fn new(ttl: i32) -> Self {
        Self {
            head: -1,
            tail: -1,
            ttl,
            next_expiration: 0,
            segments: 0,
            next_to_merge: -1,
            last_cutoff_freq: 0,
            _pad: [0; 36],
        }
    }

    pub fn head(&self) -> Option<i32> {
        self.head.as_option()
    }

    pub fn set_head(&mut self, id: Option<i32>) {
        self.head = id.unwrap_or(-1);
    }

    pub fn next_to_merge(&self) -> Option<i32> {
        self.next_to_merge.as_option()
    }

    pub fn set_next_to_merge(&mut self, next: Option<i32>) {
        self.next_to_merge = next.unwrap_or(-1);
    }

    // expire segments from this TtlBucket, returns the number of segments expired
    fn expire<S: BuildHasher>(
        &mut self,
        hashtable: &mut HashTable<S>,
        segments: &mut Segments,
    ) -> usize {
        if self.head.is_none() {
            return 0;
        }

        // this is intended to let a slow client finish writing to the expiring
        // segment.
        // TODO(bmartin): is this needed in this design?
        let grace_period = CoarseDuration::from_secs(2);

        let mut expired = 0;

        loop {
            let seg_id = self.head;
            if seg_id < 0 {
                return expired;
            }
            let flush_at = segments.flush_at();
            let mut segment = segments.get_mut(seg_id).unwrap();
            if segment.create_at() + segment.ttl() + grace_period < Instant::recent()
                || segment.create_at() < flush_at
            {
                if let Some(next) = segment.next_seg() {
                    self.head = next;
                } else {
                    self.head = -1;
                    self.tail = -1;
                }
                let _ = segment.clear(hashtable, true);
                segments.push_free(seg_id);
                increment_counter!(&Stat::SegmentExpire);
                expired += 1;
            } else {
                return expired;
            }
        }
    }

    fn try_expand(&mut self, segments: &mut Segments) -> Result<(), Error> {
        if let Some(id) = segments.pop_free() {
            {
                if self.tail.is_some() {
                    let tail = segments.get_mut(self.tail).unwrap();
                    tail.header.set_next_seg(id);
                }
            }

            let segment = segments.get_mut(id).unwrap();
            segment.header.set_prev_seg(self.tail);
            segment.header.set_next_seg(-1);
            segment
                .header
                .set_ttl(CoarseDuration::from_secs(self.ttl as u32));
            if self.head.is_none() {
                debug_assert!(self.tail.is_none());
                self.head = id;
            }
            self.tail = id;
            self.segments += 1;
            debug_assert_eq!(segment.header.evictable(), false);
            segment.header.set_evictable(true);
            segment.header.set_accessible(true);
            Ok(())
        } else {
            Err(Error::NoFreeSegments)
        }
    }

    pub(crate) fn reserve(
        &mut self,
        size: usize,
        segments: &mut Segments,
    ) -> Result<ReservedItem, Error> {
        trace!("reserving: {} bytes for ttl: {}", size, self.ttl);

        let seg_size = segments.segment_size() as usize;

        if size > seg_size {
            debug!("item is oversized");
            return Err(Error::ItemOversized);
        }

        loop {
            if let Ok(segment) = segments.get_mut(self.tail) {
                if !segment.accessible() {
                    continue;
                }
                // TODO(bmartin): this handling needs to change for threaded impl
                let offset = segment.header.write_offset() as usize;
                debug!("offset: {}", offset);
                if offset + size <= seg_size {
                    let size = size as i32;
                    let _ = segment.header.incr_write_offset(size);
                    let _ = segment.header.incr_occupied_size(size);
                    increment_gauge!(&Stat::ItemCurrent);
                    increment_gauge_by!(&Stat::ItemCurrentBytes, size as i64);
                    segment.header.incr_n_item();
                    let ptr = unsafe { segment.data.as_mut_ptr().add(offset) };

                    let item = RawItem::from_ptr(ptr);
                    return Ok(ReservedItem::new(item, segment.header.id(), offset));
                }
            }
            self.try_expand(segments)?;
        }
    }
}

impl Default for TtlBuckets {
    fn default() -> Self {
        Self::new()
    }
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
            std::cmp::min(
                (ttl >> TTL_BUCKET_INTERVAL_N_BIT_4) as usize + N_BUCKET_PER_STEP * 3,
                self.buckets.len() - 1,
            )
        }
    }

    // TODO(bmartin): confirm handling for negative TTLs here...
    pub(crate) fn get_mut_bucket(&mut self, ttl: CoarseDuration) -> &mut TtlBucket {
        let index = self.get_bucket_index(ttl);

        // NOTE: since get_bucket_index() must return an index within the slice,
        // we do not need to worry about UB here.
        unsafe { self.buckets.get_unchecked_mut(index) }
    }

    pub(crate) fn expire<S: BuildHasher>(
        &mut self,
        hashtable: &mut HashTable<S>,
        segments: &mut Segments,
    ) -> usize {
        let mut expired = 0;
        for bucket in self.buckets.iter_mut() {
            expired += bucket.expire(hashtable, segments);
        }
        expired
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bucket_index() {
        let ttl_buckets = TtlBuckets::new();
        assert_eq!(ttl_buckets.get_bucket_index(CoarseDuration::ZERO), 1023);
        assert_eq!(ttl_buckets.get_bucket_index(CoarseDuration::MAX), 1023);
    }
}
