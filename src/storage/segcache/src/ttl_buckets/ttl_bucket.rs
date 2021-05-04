// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! TTL bucket containing a segment chain which stores items with a similar TTL
//! in an ordered fashion.
//!
//! TTL Bucket:
//! ```text
//! ┌──────────────┬──────────────┬─────────────┬──────────────┐
//! │   HEAD SEG   │   TAIL SEG   │     TTL     │   SEGMENTS   │
//! │              │              │             │              │
//! │    32 bit    │    32 bit    │    32 bit   │    32 bit    │
//! ├──────────────┼──────────────┴─────────────┴──────────────┤
//! │  NEXT MERGE  │                  PADDING                  │
//! │              │                                           │
//! │    32 bit    │                  96 bit                   │
//! ├──────────────┴───────────────────────────────────────────┤
//! │                         PADDING                          │
//! │                                                          │
//! │                         128 bit                          │
//! ├──────────────────────────────────────────────────────────┤
//! │                         PADDING                          │
//! │                                                          │
//! │                         128 bit                          │
//! └──────────────────────────────────────────────────────────┘
//! ```

use crate::*;
use crate::common::ThinOption;

/// Each ttl bucket contains a segment chain to store items with a similar TTL
/// in an ordered fashion. The first segment to expire will be the head of the
/// segment chain. This allows us to efficiently scan across the [`TtlBuckets`]
/// and expire segments in an eager fashion.
pub struct TtlBucket {
    head: i32,
    tail: i32,
    ttl: i32,
    segments: i32,
    next_to_merge: i32,
    _pad: [u8; 44],
}


#[cfg(feature = "dump")]
#[derive(Serialize, Deserialize)]
pub struct TtlBucketDump {
    ttl: i32,
    head: i32,
}

impl TtlBucket {
    pub(super) fn new(ttl: i32) -> Self {
        Self {
            head: -1,
            tail: -1,
            ttl,
            segments: 0,
            next_to_merge: -1,
            _pad: [0; 44],
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
    pub(super) fn expire(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
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
            if segment.create_at() + segment.ttl() + grace_period < CoarseInstant::recent()
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

    fn try_expand(&mut self, segments: &mut Segments) -> Result<(), TtlBucketsError> {
        if let Some(id) = segments.pop_free() {
            {
                if self.tail.is_some() {
                    let mut tail = segments.get_mut(self.tail).unwrap();
                    tail.set_next_seg(id);
                }
            }

            let mut segment = segments.get_mut(id).unwrap();
            segment.set_prev_seg(self.tail);
            segment.set_next_seg(-1);
            segment.set_ttl(CoarseDuration::from_secs(self.ttl as u32));
            if self.head.is_none() {
                debug_assert!(self.tail.is_none());
                self.head = id;
            }
            self.tail = id;
            self.segments += 1;
            debug_assert_eq!(segment.evictable(), false);
            segment.set_evictable(true);
            segment.set_accessible(true);
            Ok(())
        } else {
            Err(TtlBucketsError::NoFreeSegments)
        }
    }

    pub(crate) fn reserve(
        &mut self,
        size: usize,
        segments: &mut Segments,
    ) -> Result<ReservedItem, TtlBucketsError> {
        trace!("reserving: {} bytes for ttl: {}", size, self.ttl);

        let seg_size = segments.segment_size() as usize;

        if size > seg_size {
            debug!("item is oversized");
            return Err(TtlBucketsError::ItemOversized { size });
        }

        loop {
            if let Ok(mut segment) = segments.get_mut(self.tail) {
                if !segment.accessible() {
                    continue;
                }
                let offset = segment.write_offset() as usize;
                trace!("offset: {}", offset);
                if offset + size <= seg_size {
                    let size = size as i32;
                    let item = segment.alloc_item(size);
                    return Ok(ReservedItem::new(item, segment.id(), offset));
                }
            }
            self.try_expand(segments)?;
        }
    }
}