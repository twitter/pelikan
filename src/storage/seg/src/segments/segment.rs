// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::{SegmentHeader, SegmentsError};
use crate::*;
use core::num::NonZeroU32;

pub const SEG_MAGIC: u64 = 0xBADC0FFEEBADCAFE;

/// A `Segment` is a contiguous allocation of bytes and an associated header
/// which contains metadata. This structure allows us to operate on mutable
/// borrows of the header and data sections to perform basic operations.
pub struct Segment<'a> {
    header: &'a mut SegmentHeader,
    data: &'a mut [u8],
}

impl<'a> Segment<'a> {
    /// Construct a `Segment` from its raw parts
    pub fn from_raw_parts(
        header: &'a mut segments::header::SegmentHeader,
        data: &'a mut [u8],
    ) -> Self {
        Segment { header, data }
    }

    /// Initialize the segment. Sets the magic bytes in the data segment (if the
    /// feature is enabled) and initializes the header fields.
    pub fn init(&mut self) {
        if cfg!(feature = "magic") {
            for (i, byte) in SEG_MAGIC.to_be_bytes().iter().enumerate() {
                self.data[i] = *byte;
            }
        }
        self.header.init();
    }

    #[cfg(feature = "magic")]
    #[inline]
    /// Reads the magic bytes from the start of the segment data section.
    pub fn magic(&self) -> u64 {
        u64::from_be_bytes([
            self.data[0],
            self.data[1],
            self.data[2],
            self.data[3],
            self.data[4],
            self.data[5],
            self.data[6],
            self.data[7],
        ])
    }

    #[inline]
    /// Checks that the magic bytes match the expected value
    ///
    /// # Panics
    ///
    /// This function will panic if the magic bytes do not match the expected
    /// value. This would indicate data corruption or that the segment was
    /// constructed from invalid data.
    pub fn check_magic(&self) {
        #[cfg(feature = "magic")]
        assert_eq!(self.magic(), SEG_MAGIC)
    }

    /// Convenience function which is used as a stop point for scanning through
    /// the segment. All valid items would exist below this value
    fn max_item_offset(&self) -> usize {
        if self.write_offset() >= ITEM_HDR_SIZE as i32 {
            std::cmp::min(self.write_offset() as usize, self.data.len()) - ITEM_HDR_SIZE
        } else if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        }
    }

    /// Check the segment integrity. This is an expensive operation. Will return
    /// a bool with a true result indicating that the segment integrity check
    /// has passed. A false result indicates that there is data corruption.
    ///
    /// # Panics
    ///
    /// This function may panic if the segment is corrupted or has been
    /// constructed from invalid bytes.
    #[cfg(feature = "debug")]
    pub(crate) fn check_integrity(&mut self, hashtable: &mut HashTable) -> bool {
        self.check_magic();

        let mut integrity = true;

        let max_offset = self.max_item_offset();
        let mut offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        let mut count = 0;

        while offset < max_offset {
            let item = RawItem::from_ptr(unsafe { self.data.as_mut_ptr().add(offset) });
            if item.klen() == 0 {
                break;
            }

            let deleted = !hashtable.is_item_at(item.key(), self.id(), offset as u64);
            if !deleted {
                count += 1;
            }
            offset += item.size();
        }

        if count != self.live_items() {
            error!(
                "seg: {} has mismatch between counted items: {} and header items: {}",
                self.id(),
                count,
                self.live_items()
            );
            integrity = false;
        }

        integrity
    }

    /// Return the segment's id
    #[inline]
    pub fn id(&self) -> NonZeroU32 {
        self.header.id()
    }

    /// Return the current write offset of the segment. This index is the start
    /// of the next write.
    #[inline]
    pub fn write_offset(&self) -> i32 {
        self.header.write_offset()
    }

    /// Set the write offset to a specific value
    #[inline]
    pub fn set_write_offset(&mut self, bytes: i32) {
        self.header.set_write_offset(bytes)
    }

    /// Return the number of live (active) bytes in the segment. This may be
    /// lower than the write offset due to items being removed/replaced
    #[inline]
    pub fn live_bytes(&self) -> i32 {
        self.header.live_bytes()
    }

    /// Return the number of live items in the segment.
    #[inline]
    pub fn live_items(&self) -> i32 {
        self.header.live_items()
    }

    /// Returns whether the segment is currently accessible from the hashtable.
    #[inline]
    pub fn accessible(&self) -> bool {
        self.header.accessible()
    }

    /// Mark whether or not the segment is accessible from the hashtable.
    #[inline]
    pub fn set_accessible(&mut self, accessible: bool) {
        self.header.set_accessible(accessible)
    }

    /// Indicate if the segment might be evictable, prefer to use `can_evict()`
    /// to check.
    #[inline]
    pub fn evictable(&self) -> bool {
        self.header.evictable()
    }

    /// Set if the segment could be considered evictable.
    #[inline]
    pub fn set_evictable(&mut self, evictable: bool) {
        self.header.set_evictable(evictable)
    }

    /// Performs some checks to determine if the segment can actually be evicted
    #[inline]
    pub fn can_evict(&self) -> bool {
        self.header.can_evict()
    }

    /// Return the segment's TTL
    #[inline]
    pub fn ttl(&self) -> Duration {
        self.header.ttl()
    }

    /// Set the segment's TTL, used when linking it into a TtlBucket
    #[inline]
    pub fn set_ttl(&mut self, ttl: Duration) {
        self.header.set_ttl(ttl)
    }

    /// Returns the time the segment was last initialized
    #[inline]
    pub fn create_at(&self) -> Instant {
        self.header.create_at()
    }

    /// Mark that the segment has been merged
    #[inline]
    pub fn mark_merged(&mut self) {
        self.header.mark_merged()
    }

    /// Return the previous segment's id. This will be a segment before it in a
    /// TtlBucket or on the free queue. A `None` indicates that this segment is
    /// the head of a bucket or the free queue.
    #[allow(dead_code)]
    #[inline]
    pub fn prev_seg(&self) -> Option<NonZeroU32> {
        self.header.prev_seg()
    }

    /// Set the previous segment id to this value. Negative values will mean
    /// that there is no previous segment, meaning this segment is the head of
    /// a bucket or the free queue
    #[inline]
    pub fn set_prev_seg(&mut self, id: Option<NonZeroU32>) {
        self.header.set_prev_seg(id)
    }

    /// Return the next segment's id. This will be a segment following it in a
    /// TtlBucket or on the free queue. A `None` indicates that this segment is
    /// the tail of a bucket or the free queue.
    #[inline]
    pub fn next_seg(&self) -> Option<NonZeroU32> {
        self.header.next_seg()
    }

    /// Set the next segment id to this value. Negative values will mean that
    /// there is no previous segment, meaning this segment is the head of a
    /// bucket or the free queue
    #[inline]
    pub fn set_next_seg(&mut self, id: Option<NonZeroU32>) {
        self.header.set_next_seg(id)
    }

    /// Decrement the live bytes by `bytes` and the live items by `1`. This
    /// would be used to update the header after an item has been removed or
    /// replaced.
    #[inline]
    pub fn decr_item(&mut self, bytes: i32) {
        self.header.decr_live_bytes(bytes);
        self.header.decr_live_items();
    }

    /// Internal function which increments the live bytes by `bytes` and the
    /// live items by `1`. Used when an item has been allocated
    #[inline]
    fn incr_item(&mut self, bytes: i32) {
        let _ = self.header.incr_write_offset(bytes);
        self.header.incr_live_bytes(bytes);
        self.header.incr_live_items();
    }

    /// Allocate a new `RawItem` with the given size
    ///
    /// # Safety
    ///
    /// This function *does not* check that there is enough free space in the
    /// segment. It is up to the caller to ensure that the resulting item fits
    /// in the segment. Data corruption or segfault is likely to occur if this
    /// is not checked.
    // TODO(bmartin): See about returning a Result here instead and avoiding the
    // potential safety issue.
    pub(crate) fn alloc_item(&mut self, size: i32) -> RawItem {
        let offset = self.write_offset() as usize;
        self.incr_item(size);
        ITEM_ALLOCATE.increment();
        ITEM_CURRENT.increment();
        ITEM_CURRENT_BYTES.add(size as _);

        let ptr = unsafe { self.data.as_mut_ptr().add(offset) };
        RawItem::from_ptr(ptr)
    }

    /// Remove an item based on its item info
    // TODO(bmartin): tombstone is currently always set
    pub(crate) fn remove_item(&mut self, item_info: u64) {
        let offset = get_offset(item_info) as usize;
        self.remove_item_at(offset)
    }

    /// Remove an item based on its offset into the segment
    pub(crate) fn remove_item_at(&mut self, offset: usize) {
        let item = self.get_item_at(offset).unwrap();

        let item_size = item.size() as i64;

        ITEM_CURRENT.decrement();
        ITEM_CURRENT_BYTES.sub(item_size);
        ITEM_DEAD.increment();
        ITEM_DEAD_BYTES.add(item_size);

        self.check_magic();
        self.decr_item(item_size as i32);
        assert!(self.live_bytes() >= 0);
        assert!(self.live_items() >= 0);

        self.check_magic();
    }

    /// Returns the item at the given offset
    // TODO(bmartin): consider changing the return type here and removing asserts?
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn get_item_at(&mut self, offset: usize) -> Option<RawItem> {
        assert!(offset <= self.max_item_offset());
        Some(RawItem::from_ptr(unsafe {
            self.data.as_mut_ptr().add(offset)
        }))
    }

    /// This is used as part of segment merging, it moves all occupied space to
    /// the beginning of the segment, leaving the end of the segment free
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn compact(&mut self, hashtable: &mut HashTable) -> Result<(), SegmentsError> {
        let max_offset = self.max_item_offset();
        let mut read_offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        let mut write_offset = read_offset;

        let mut items_pruned = 0;
        let mut bytes_pruned = 0;

        while read_offset <= max_offset {
            let item = self.get_item_at(read_offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            let item_size = item.size();

            // don't copy deleted items
            let deleted = !hashtable.is_item_at(item.key(), self.id(), read_offset as u64);
            if deleted {
                items_pruned += 1;
                bytes_pruned += item.size();
                // move read offset forward, leave write offset trailing
                read_offset += item_size;
                ITEM_COMPACTED.increment();
                continue;
            }

            // only copy if the offsets are different
            if read_offset != write_offset {
                let src = unsafe { self.data.as_ptr().add(read_offset) };
                let dst = unsafe { self.data.as_mut_ptr().add(write_offset) };

                if hashtable
                    .relink_item(
                        item.key(),
                        self.id(),
                        self.id(),
                        read_offset as u64,
                        write_offset as u64,
                    )
                    .is_ok()
                {
                    // note that we use a copy that can handle overlap
                    unsafe {
                        std::ptr::copy(src, dst, item_size);
                    }
                } else {
                    // this shouldn't happen, but if relink does fail we can
                    // only move forward or return an error
                    read_offset += item_size;
                    write_offset = read_offset;
                    continue;
                }
            }

            read_offset += item_size;
            write_offset += item_size;
            continue;
        }

        // We have removed dead items, so we must subtract the pruned items from
        // the dead item stats.
        ITEM_DEAD.sub(items_pruned as _);
        ITEM_DEAD_BYTES.sub(bytes_pruned as _);

        // updates the write offset to the new position
        self.set_write_offset(write_offset as i32);

        Ok(())
    }

    /// This is used to copy data from this segment into the target segment and
    /// relink the items in the hashtable
    ///
    /// # NOTE
    ///
    /// Any items that don't fit in the target will be left in this segment it
    /// is left to the caller to decide how to handle this
    pub(crate) fn copy_into(
        &mut self,
        target: &mut Segment,
        hashtable: &mut HashTable,
    ) -> Result<(), SegmentsError> {
        let max_offset = self.max_item_offset();
        let mut read_offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        let mut items_copied = 0;
        let mut bytes_copied = 0;

        while read_offset <= max_offset {
            let item = self.get_item_at(read_offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            let item_size = item.size();

            let write_offset = target.write_offset() as usize;

            // skip deleted items and ones that won't fit in the target segment
            let deleted = !hashtable.is_item_at(item.key(), self.id(), read_offset as u64);
            if deleted || write_offset + item_size >= target.data.len() {
                read_offset += item_size;
                continue;
            }

            let src = unsafe { self.data.as_ptr().add(read_offset) };
            let dst = unsafe { target.data.as_mut_ptr().add(write_offset) };

            if hashtable
                .relink_item(
                    item.key(),
                    self.id(),
                    target.id(),
                    read_offset as u64,
                    write_offset as u64,
                )
                .is_ok()
            {
                // since we're working with two different segments, we can use
                // nonoverlapping copy
                unsafe {
                    std::ptr::copy_nonoverlapping(src, dst, item_size);
                }
                self.remove_item_at(read_offset);
                target.header.incr_live_items();
                target.header.incr_live_bytes(item_size as i32);
                target.set_write_offset(write_offset as i32 + item_size as i32);
                items_copied += 1;
                bytes_copied += item_size;
            } else {
                // TODO(bmartin): figure out if this could happen and make the
                // relink function infallible if it can't happen
                return Err(SegmentsError::RelinkFailure);
            }

            read_offset += item_size;
        }

        // We need to increment the current bytes, because removing items from
        // this segment decrements these as it marks the item as removed. This
        // should result in these stats remaining unchanged by this function.
        ITEM_CURRENT.add(items_copied);
        ITEM_CURRENT_BYTES.add(bytes_copied as _);

        Ok(())
    }

    /// This is used as part of segment merging, it removes items from the
    /// segment based on a cutoff frequency and target ratio. Since the cutoff
    /// frequency is adjusted, it is returned as the result.
    pub(crate) fn prune(
        &mut self,
        hashtable: &mut HashTable,
        cutoff_freq: f64,
        target_ratio: f64,
    ) -> f64 {
        let max_offset = self.max_item_offset();
        let mut offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        let to_keep = (self.data.len() as f64 * target_ratio).floor() as i32;
        let to_drop = self.live_bytes() - to_keep;

        let mut n_scanned = 0;
        let mut n_dropped = 0;
        let mut n_retained = 0;

        let mean_size = self.live_bytes() as f64 / self.live_items() as f64;
        let mut cutoff = (1.0 + cutoff_freq) / 2.0;
        let mut n_th_update = 1;
        let update_interval = self.data.len() / 10;

        while offset <= max_offset {
            let item = self.get_item_at(offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            let item_size = item.size();

            let deleted = !hashtable.is_item_at(item.key(), self.id(), offset as u64);
            if deleted {
                // do we need to evict again here? Why is that done in the C code?
                offset += item_size;
                continue;
            }

            n_scanned += item_size;

            if n_scanned >= (n_th_update * update_interval) {
                n_th_update += 1;
                // magical formula for adjusting cutoff based on retention,
                // scan progress, and target ratio
                let t = ((n_retained as f64) / (n_scanned as f64) - target_ratio) / target_ratio;
                if !(-0.5..=0.5).contains(&t) {
                    cutoff *= 1.0 + t;
                }
                trace!("cutoff adj to: {}", cutoff);
            }

            let item_frequency =
                hashtable.get_freq(item.key(), self, offset as u64).unwrap() as f64;
            let weighted_frequency = item_frequency / (item_size as f64 / mean_size);

            if cutoff >= 0.0001
                && to_drop > 0
                && n_dropped < to_drop as usize
                && weighted_frequency <= cutoff
            {
                trace!(
                    "evicting item size: {} freq: {} w_freq: {} cutoff: {}",
                    item_size,
                    item_frequency,
                    weighted_frequency,
                    cutoff
                );
                if !hashtable.evict(item.key(), offset.try_into().unwrap(), self) {
                    // this *shouldn't* happen, but to keep header integrity, we
                    // warn and remove the item even if it wasn't in the
                    // hashtable
                    warn!("unlinked item was present in segment");
                    self.remove_item_at(offset);
                }
                n_dropped += item_size;
                offset += item_size;
                continue;
            } else {
                trace!(
                    "keeping item size: {} freq: {} w_freq: {} cutoff: {}",
                    item_size,
                    item_frequency,
                    weighted_frequency,
                    cutoff
                );
            }

            offset += item_size;
            n_retained += item_size;
        }

        cutoff
    }

    /// Remove all items from the segment, unlinking them from the hashtable.
    /// If expire is true, this is treated as an expiration option. Otherwise it
    /// is treated as an eviction.
    pub(crate) fn clear(&mut self, hashtable: &mut HashTable, expire: bool) {
        self.set_accessible(false);
        self.set_evictable(false);

        let max_offset = self.max_item_offset();
        let mut offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        // track all items and bytes that are cleared
        let mut items = 0;
        let mut bytes = 0;

        while offset <= max_offset {
            let item = self.get_item_at(offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            debug_assert!(item.klen() > 0, "invalid klen: ({})", item.klen());

            items += 1;
            bytes += item.size();

            let deleted = !hashtable.is_item_at(item.key(), self.id(), offset as u64);
            if !deleted {
                trace!("evicting from hashtable");
                let removed = if expire {
                    hashtable.expire(item.key(), offset.try_into().unwrap(), self)
                } else {
                    hashtable.evict(item.key(), offset.try_into().unwrap(), self)
                };
                if !removed {
                    // this *shouldn't* happen, but to keep header integrity, we
                    // warn and remove the item even if it wasn't in the
                    // hashtable
                    warn!("unlinked item was present in segment");
                    self.remove_item_at(offset);
                }
            }

            debug_assert!(
                self.live_items() >= 0,
                "cleared segment has invalid number of live items: ({})",
                self.live_items()
            );
            debug_assert!(
                self.live_bytes() >= 0,
                "cleared segment has invalid number of live bytes: ({})",
                self.live_bytes()
            );
            offset += item.size();
        }

        // At the end of the clear phase above, we have only dead items that we
        // are clearing from the segment. The functions that removed the live
        // items from the hashtable have decremented the live items, and
        // incremented the dead items. So we subtract all items that were in
        // this segment from the dead item stats.
        ITEM_DEAD.sub(items as _);
        ITEM_DEAD_BYTES.sub(bytes as _);

        // skips over seg_wait_refcount and evict retry, because no threading

        if self.live_items() > 0 {
            error!(
                "segment not empty after clearing, still contains: {} items",
                self.live_items()
            );
            panic!();
        }

        let expected_size = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC) as i32
        } else {
            0
        };
        if self.live_bytes() != expected_size {
            error!("segment size incorrect after clearing");
            panic!();
        }

        self.set_write_offset(self.live_bytes());
    }
}

#[cfg(feature = "magic")]
impl<'a> std::fmt::Debug for Segment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("Segment")
            .field("header", &self.header)
            .field("magic", &format!("0x{:X}", self.magic()))
            .field("data", &format!("{:02X?}", self.data))
            .finish()
    }
}

#[cfg(not(feature = "magic"))]
impl<'a> std::fmt::Debug for Segment<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        f.debug_struct("Segment")
            .field("header", &self.header)
            .field("data", &format!("{:X?}", self.data))
            .finish()
    }
}
