use crate::common::ThinOption;
use crate::SEG_MAGIC;
use rustcommon_time::*;

pub const SEG_HDR_SIZE: usize = std::mem::size_of::<SegmentHeader>();

use rustcommon_time::CoarseDuration as Duration;
use rustcommon_time::CoarseInstant as Instant;

#[derive(Debug)]
#[repr(C)]
/// The `SegmentHeader` contains metadata about the segment. It is intended to
/// be stored in DRAM as the fields are frequently accessed and changed.
pub struct SegmentHeader {
    id: i32,
    write_offset: i32,
    occupied_size: i32,
    n_item: i32,
    prev_seg: i32,
    next_seg: i32,
    n_hit: i32,
    n_active_items: i32,
    n_active_bytes: i32,
    last_merge_epoch: i16,
    create_at: Instant,
    ttl: u32,
    merge_at: Instant,
    accessible: bool,
    evictable: bool,
    recovered: bool,
    unused: u16,
    _pad: [u8; 4],
}

impl SegmentHeader {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            write_offset: 0,
            occupied_size: 0,
            n_item: 0,
            prev_seg: -1,
            next_seg: -1,
            n_hit: 0,
            n_active_items: 0,
            n_active_bytes: 0,
            last_merge_epoch: -1,
            create_at: Instant::recent(),
            ttl: 0,
            merge_at: Instant::recent(),
            accessible: false,
            evictable: false,
            recovered: false,
            unused: 0,
            _pad: [0; 4],
        }
    }

    pub fn init(&mut self) {
        // TODO(bmartin): should these be `debug_assert` or are we enforcing
        // invariants? Eitherway, keeping them before changing values in the
        // header is probably wise?
        assert!(!self.accessible());
        assert!(!self.evictable());

        self.reset();

        self.prev_seg = -1;
        self.next_seg = -1;
        self.n_item = 0;
        self.create_at = Instant::recent();
        self.merge_at = Instant::recent();
        self.accessible = true;
        self.n_hit = 0;
        self.last_merge_epoch = 0;
        self.n_active_items = 0;
        self.n_active_bytes = 0;
    }

    // TODO(bmartin): maybe have some debug_assert for n_item == 0 ?
    pub fn reset(&mut self) {
        let offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC) as i32
        } else {
            0
        };

        self.write_offset = offset;
        self.occupied_size = offset;
    }

    #[inline]
    pub fn id(&self) -> i32 {
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
    pub fn n_item(&self) -> i32 {
        self.n_item
    }

    #[inline]
    /// Increment the number of live items.
    pub fn incr_n_item(&mut self) {
        self.n_item += 1;
    }

    #[inline]
    /// Decrement the number of live items.
    pub fn decr_n_item(&mut self) {
        self.n_item -= 1;
    }

    #[inline]
    /// Returns the TTL for the segment.
    pub fn ttl(&self) -> CoarseDuration {
        CoarseDuration::from_secs(self.ttl)
    }

    #[inline]
    /// Sets the TTL for the segment.
    pub fn set_ttl(&mut self, ttl: CoarseDuration) {
        self.ttl = ttl.as_secs();
    }

    #[inline]
    /// The number of bytes used in the segment.
    pub fn occupied_size(&self) -> i32 {
        self.occupied_size
    }

    #[inline]
    /// Increment the number of bytes used in the segment.
    pub fn incr_occupied_size(&mut self, bytes: i32) -> i32 {
        let prev = self.occupied_size;
        self.occupied_size += bytes;
        prev
    }

    #[inline]
    /// Decrement the number of bytes used in the segment.
    pub fn decr_occupied_size(&mut self, bytes: i32) -> i32 {
        let prev = self.occupied_size;
        self.occupied_size -= bytes;
        prev
    }

    #[inline]
    /// Returns an option containing the previous segment id if there is one.
    pub fn prev_seg(&self) -> Option<i32> {
        self.prev_seg.as_option()
    }

    #[inline]
    /// Set the previous segment to some id. Passing a negative id results in
    /// clearing the previous segment pointer.
    pub fn set_prev_seg(&mut self, id: i32) {
        self.prev_seg = id;
    }

    #[inline]
    /// Returns an option containing the next segment id if there is one.
    pub fn next_seg(&self) -> Option<i32> {
        self.next_seg.as_option()
    }

    #[inline]
    /// Set the next segment to some id. Passing a negative id results in
    /// clearing the next segment pointer.
    pub fn set_next_seg(&mut self, id: i32) {
        self.next_seg = id;
    }

    #[inline]
    /// Returns the instant at which the segment was created
    pub fn create_at(&self) -> CoarseInstant {
        self.create_at
    }

    #[inline]
    /// Update the created time
    pub fn mark_created(&mut self) {
        self.create_at = CoarseInstant::recent();
    }

    #[inline]
    /// Returns the instant at which the segment was merged
    pub fn merge_at(&self) -> CoarseInstant {
        self.merge_at
    }

    #[inline]
    /// Update the created time
    pub fn mark_merged(&mut self) {
        self.merge_at = CoarseInstant::recent();
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
            && (self.create_at() + self.ttl()) >= (Instant::recent() + Duration::from_secs(5))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(SEG_HDR_SIZE, 64);
        assert_eq!(std::mem::size_of::<SegmentHeader>(), SEG_HDR_SIZE)
    }
}
