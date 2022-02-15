// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! TTL bucket containing a segment chain which stores items with a similar TTL
//! in an ordered fashion.
//!
//! TTL Bucket:
//! ```text
//! ┌──────────────┬──────────────┬─────────────┬──────────────┐
//! │   HEAD SEG   │   TAIL SEG   │     TTL     │     NSEG     │
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

use super::{SEGMENT_CLEAR, SEGMENT_EXPIRE};
use crate::*;
use core::num::NonZeroU32;

/// Each ttl bucket contains a segment chain to store items with a similar TTL
/// in an ordered fashion. The first segment to expire will be the head of the
/// segment chain. This allows us to efficiently scan across the [`TtlBuckets`]
/// and expire segments in an eager fashion.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct TtlBucket {
    head: Option<NonZeroU32>,
    tail: Option<NonZeroU32>,
    ttl: i32,
    nseg: i32,
    next_to_merge: Option<NonZeroU32>,
    _pad: [u8; 44],
}

impl TtlBucket {
    /// Create a new `TtlBucket` which will hold items with the provided TTL.
    pub(super) fn new(ttl: i32) -> Self {
        Self {
            head: None,
            tail: None,
            ttl,
            nseg: 0,
            next_to_merge: None,
            _pad: [0; 44],
        }
    }

    /// Returns the segment ID of the head of the `TtlBucket`.
    pub fn head(&self) -> Option<NonZeroU32> {
        self.head
    }

    /// Set the segment ID of the head of the `TtlBucket`.
    pub fn set_head(&mut self, id: Option<NonZeroU32>) {
        self.head = id;
    }

    /// Returns the segment ID of the next segment to merge within the
    /// `TtlBucket`.
    pub fn next_to_merge(&self) -> Option<NonZeroU32> {
        self.next_to_merge
    }

    /// Set the next segment to be merged within the `TtlBucket`.
    pub fn set_next_to_merge(&mut self, next: Option<NonZeroU32>) {
        self.next_to_merge = next;
    }

    /// Expire segments from this TtlBucket, returns the number of segments
    /// expired.
    pub(super) fn expire(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
        if self.head.is_none() {
            return 0;
        }

        let mut expired = 0;

        loop {
            let seg_id = self.head;
            if let Some(seg_id) = seg_id {
                let flush_at = segments.flush_at();
                let mut segment = segments.get_mut(seg_id).unwrap();
                if segment.create_at() + segment.ttl() <= Instant::recent()
                    || segment.create_at() < flush_at
                {
                    if let Some(next) = segment.next_seg() {
                        self.head = Some(next);
                    } else {
                        self.head = None;
                        self.tail = None;
                    }
                    let _ = segment.clear(hashtable, true);
                    segments.push_free(seg_id);
                    SEGMENT_EXPIRE.increment();
                    expired += 1;
                } else {
                    return expired;
                }
            } else {
                return expired;
            }
        }
    }

    /// Clear segments from this TtlBucket, returns the number of segments
    /// expired.
    pub(super) fn clear(&mut self, hashtable: &mut HashTable, segments: &mut Segments) -> usize {
        if self.head.is_none() {
            return 0;
        }

        let mut cleared = 0;

        loop {
            let seg_id = self.head;
            if let Some(seg_id) = seg_id {
                let mut segment = segments.get_mut(seg_id).unwrap();
                if let Some(next) = segment.next_seg() {
                    self.head = Some(next);
                } else {
                    self.head = None;
                    self.tail = None;
                }
                let _ = segment.clear(hashtable, true);
                segments.push_free(seg_id);
                SEGMENT_CLEAR.increment();
                cleared += 1;
            } else {
                return cleared;
            }
        }
    }

    /// Attempts to expand the `TtlBucket` by allocating a segment from the free
    /// queue. If there are no segments currently free, this function will
    /// return and error. It is up to the caller to handle the error and retry.
    fn try_expand(&mut self, segments: &mut Segments) -> Result<(), TtlBucketsError> {
        if let Some(id) = segments.pop_free() {
            {
                if let Some(tail_id) = self.tail {
                    let mut tail = segments.get_mut(tail_id).unwrap();
                    tail.set_next_seg(Some(id));
                }
            }

            let mut segment = segments.get_mut(id).unwrap();
            segment.set_prev_seg(self.tail);
            segment.set_next_seg(None);
            segment.set_ttl(Duration::from_secs(self.ttl as u32));
            if self.head.is_none() {
                debug_assert!(self.tail.is_none());
                self.head = Some(id);
            }
            self.tail = Some(id);
            self.nseg += 1;
            debug_assert!(!segment.evictable(), "segment should not be evictable");
            segment.set_evictable(true);
            segment.set_accessible(true);
            Ok(())
        } else {
            Err(TtlBucketsError::NoFreeSegments)
        }
    }

    /// Reserve space in this `TtlBucket` for an item with the specified size in
    /// bytes. This function will return an error if the item is oversized, or
    /// if there is no space in the `TtlBucket` for the item and the `TtlBucket`
    /// could not be expanded with a segment from the free queue.
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
            if let Some(id) = self.tail {
                if let Ok(mut segment) = segments.get_mut(id) {
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
            }
            self.try_expand(segments)?;
        }
    }
}
