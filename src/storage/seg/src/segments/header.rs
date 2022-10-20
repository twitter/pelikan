// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! The `SegmentHeader` contains metadata about the segment. It is intended to
//! be stored in DRAM as the fields are frequently accessed and changed.
//!
//! The header is padded out to occupy a full cacheline
//! ```text
//! ┌──────────────┬──────────────┬──────────────┬──────────────┐
//! │      ID      │ WRITE OFFSET │  LIVE BYTES  │  LIVE ITEMS  │
//! │              │              │              │              │
//! │    32 bit    │    32 bit    │    32 bit    │    32 bit    │
//! ├──────────────┼──────────────┼──────────────┼──────────────┤
//! │   PREV SEG   │   NEXT SEG   │  CREATE AT   │   MERGE AT   │
//! │              │              │              │              │
//! │    32 bit    │    32 bit    │    32 bit    │    32 bit    │
//! ├──────────────┼──┬──┬────────┴──────────────┴──────────────┤
//! │     TTL      │  │  │               PADDING                │   Accessible
//! │              │  │◀─┼──────────────────────────────────────┼──    8 bit
//! │    32 bit    │8b│8b│                80 bit                │
//! ├──────────────┴──┴──┴──────────────────────────────────────┤    Evictable
//! │                          PADDING                          │      8 bit
//! │                                                           │
//! │                          128 bit                          │
//! └───────────────────────────────────────────────────────────┘
//! ```

use super::SEG_MAGIC;
use core::num::NonZeroU32;

use crate::*;

// the minimum age of a segment before it is eligible for eviction
// TODO(bmartin): this should be parameterized.
const SEG_MATURE_TIME: Duration = Duration::from_secs(20);

#[derive(Debug)]
#[repr(C)]
pub struct SegmentHeader {
    /// The id for this segment
    id: NonZeroU32,
    /// Current write position
    write_offset: i32,
    /// The number of live bytes in the segment
    live_bytes: i32,
    /// The number of live items in the segment
    live_items: i32,
    /// The previous segment in the TtlBucket or on the free queue
    prev_seg: Option<NonZeroU32>,
    /// The next segment in the TtlBucket or on the free queue
    next_seg: Option<NonZeroU32>,
    /// The time the segment was last "created" (taken from free queue)
    create_at: Instant,
    /// The time the segment was last merged
    merge_at: Instant,
    /// The TTL of the segment in seconds
    ttl: u32,
    /// Is the segment accessible?
    accessible: bool,
    /// Is the segment evictable?
    evictable: bool,
    _pad: [u8; 25],
}

impl SegmentHeader {
    pub fn new(id: NonZeroU32) -> Self {
        let now = Instant::now();
        Self {
            id,
            write_offset: 0,
            live_bytes: 0,
            live_items: 0,
            prev_seg: None,
            next_seg: None,
            create_at: now,
            ttl: 0,
            merge_at: now,
            accessible: false,
            evictable: false,
            _pad: [0; 25],
        }
    }

    pub fn init(&mut self) {
        // TODO(bmartin): should these be `debug_assert` or are we enforcing
        // invariants? Eitherway, keeping them before changing values in the
        // header is probably wise?
        assert!(!self.accessible());
        assert!(!self.evictable());

        self.reset();
        let now = Instant::now();

        self.prev_seg = None;
        self.next_seg = None;
        self.live_items = 0;
        self.create_at = now;
        self.merge_at = now;
        self.accessible = true;
    }

    // TODO(bmartin): maybe have some debug_assert for n_item == 0 ?
    pub fn reset(&mut self) {
        let offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC) as i32
        } else {
            0
        };

        self.write_offset = offset;
        self.live_bytes = offset;
    }

    #[inline]
    pub fn id(&self) -> NonZeroU32 {
        self.id
    }

    #[inline]
    /// Returns the offset in the segment to begin writing the next item.
    pub fn write_offset(&self) -> i32 {
        self.write_offset
    }

    #[inline]
    /// Sets the write offset to the provided value. Typically used when
    /// resetting the segment.
    pub fn set_write_offset(&mut self, bytes: i32) {
        self.write_offset = bytes;
    }

    #[inline]
    /// Moves the write offset forward by some number of bytes and returns the
    /// previous value. This is used as part of writing a new item to reserve
    /// some number of bytes and return the position to begin writing.
    pub fn incr_write_offset(&mut self, bytes: i32) -> i32 {
        let prev = self.write_offset;
        self.write_offset += bytes;
        prev
    }

    #[inline]
    /// Is the segment accessible?
    pub fn accessible(&self) -> bool {
        self.accessible
    }

    #[inline]
    /// Set whether the segment is accessible.
    pub fn set_accessible(&mut self, accessible: bool) {
        self.accessible = accessible;
    }

    #[inline]
    /// Is the segment evictable?
    pub fn evictable(&self) -> bool {
        self.evictable
    }

    #[inline]
    /// Set whether the segment is evictable.
    pub fn set_evictable(&mut self, evictable: bool) {
        self.evictable = evictable;
    }

    #[inline]
    /// The number of live items within the segment.
    pub fn live_items(&self) -> i32 {
        self.live_items
    }

    #[inline]
    /// Increment the number of live items.
    pub fn incr_live_items(&mut self) {
        self.live_items += 1;
    }

    #[inline]
    /// Decrement the number of live items.
    pub fn decr_live_items(&mut self) {
        self.live_items -= 1;
    }

    #[inline]
    /// Returns the TTL for the segment.
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.ttl)
    }

    #[inline]
    /// Sets the TTL for the segment.
    pub fn set_ttl(&mut self, ttl: Duration) {
        self.ttl = ttl.as_secs();
    }

    #[inline]
    /// The number of bytes used in the segment.
    pub fn live_bytes(&self) -> i32 {
        self.live_bytes
    }

    #[inline]
    /// Increment the number of bytes used in the segment.
    pub fn incr_live_bytes(&mut self, bytes: i32) -> i32 {
        let prev = self.live_bytes;
        self.live_bytes += bytes;
        prev
    }

    #[inline]
    /// Decrement the number of bytes used in the segment.
    pub fn decr_live_bytes(&mut self, bytes: i32) -> i32 {
        let prev = self.live_bytes;
        self.live_bytes -= bytes;
        prev
    }

    #[inline]
    /// Returns an option containing the previous segment id if there is one.
    pub fn prev_seg(&self) -> Option<NonZeroU32> {
        self.prev_seg
    }

    #[inline]
    /// Set the previous segment to some id. Passing a negative id results in
    /// clearing the previous segment pointer.
    pub fn set_prev_seg(&mut self, id: Option<NonZeroU32>) {
        self.prev_seg = id;
    }

    #[inline]
    /// Returns an option containing the next segment id if there is one.
    pub fn next_seg(&self) -> Option<NonZeroU32> {
        self.next_seg
    }

    #[inline]
    /// Set the next segment to some id. Passing a negative id results in
    /// clearing the next segment pointer.
    pub fn set_next_seg(&mut self, id: Option<NonZeroU32>) {
        self.next_seg = id;
    }

    #[inline]
    /// Returns the instant at which the segment was created
    pub fn create_at(&self) -> Instant {
        self.create_at
    }

    #[inline]
    /// Update the created time
    pub fn mark_created(&mut self) {
        self.create_at = Instant::now();
    }

    #[inline]
    /// Returns the instant at which the segment was merged
    pub fn merge_at(&self) -> Instant {
        self.merge_at
    }

    #[inline]
    /// Update the created time
    pub fn mark_merged(&mut self) {
        self.merge_at = Instant::now();
    }

    #[inline]
    // clippy throws a false positive for suspicious_operation_groupings lint
    // for the instant + duration portion. We set the allow pragma to silence
    // the false positive.
    #[allow(clippy::suspicious_operation_groupings)]
    /// Can the segment be evicted?
    pub fn can_evict(&self) -> bool {
        self.evictable()
            && self.next_seg().is_some()
            && (self.create_at() + self.ttl()) >= (Instant::now() + SEG_MATURE_TIME)
    }
}
