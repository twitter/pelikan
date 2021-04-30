// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use super::{SegmentHeader, SegmentsError};
use crate::*;

use serde::{Deserialize, Serialize};

pub struct Segment<'a> {
    header: &'a mut SegmentHeader,
    data: &'a mut [u8],
}

impl<'a> Segment<'a> {
    pub fn from_raw_parts(
        header: &'a mut segments::header::SegmentHeader,
        data: &'a mut [u8],
    ) -> Self {
        Segment { header, data }
    }

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
    pub fn check_magic(&self) {
        #[cfg(feature = "magic")]
        assert_eq!(self.magic(), SEG_MAGIC)
    }

    fn max_item_offset(&self) -> usize {
        if self.write_offset() >= ITEM_HDR_SIZE as i32 {
            std::cmp::min(self.write_offset() as usize, self.data.len()) - ITEM_HDR_SIZE
        } else if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        }
    }

    pub fn check_integrity(&mut self) -> bool {
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
            if !item.deleted() {
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

    #[inline]
    pub fn id(&self) -> i32 {
        self.header.id()
    }

    #[inline]
    pub fn write_offset(&self) -> i32 {
        self.header.write_offset()
    }

    #[inline]
    pub fn set_write_offset(&mut self, bytes: i32) {
        self.header.set_write_offset(bytes)
    }

    #[inline]
    pub fn live_bytes(&self) -> i32 {
        self.header.live_bytes()
    }

    #[inline]
    pub fn live_items(&self) -> i32 {
        self.header.live_items()
    }

    #[inline]
    pub fn accessible(&self) -> bool {
        self.header.accessible()
    }

    #[inline]
    pub fn set_accessible(&mut self, accessible: bool) {
        self.header.set_accessible(accessible)
    }

    #[inline]
    pub fn evictable(&self) -> bool {
        self.header.evictable()
    }

    #[inline]
    pub fn set_evictable(&mut self, evictable: bool) {
        self.header.set_evictable(evictable)
    }

    #[inline]
    pub fn can_evict(&self) -> bool {
        self.header.can_evict()
    }

    #[inline]
    pub fn set_ttl(&mut self, ttl: CoarseDuration) {
        self.header.set_ttl(ttl)
    }

    #[inline]
    pub fn create_at(&self) -> CoarseInstant {
        self.header.create_at()
    }

    #[inline]
    pub fn mark_merged(&mut self) {
        self.header.mark_merged()
    }

    #[inline]
    pub fn ttl(&self) -> CoarseDuration {
        self.header.ttl()
    }

    #[inline]
    pub fn prev_seg(&self) -> Option<i32> {
        self.header.prev_seg()
    }

    #[inline]
    pub fn next_seg(&self) -> Option<i32> {
        self.header.next_seg()
    }

    #[inline]
    pub fn set_prev_seg(&mut self, id: i32) {
        self.header.set_prev_seg(id)
    }

    #[inline]
    pub fn set_next_seg(&mut self, id: i32) {
        self.header.set_next_seg(id)
    }

    #[inline]
    pub fn decr_item(&mut self, bytes: i32) {
        self.header.decr_live_bytes(bytes);
        self.header.decr_live_items();
    }

    #[inline]
    fn incr_item(&mut self, bytes: i32) {
        let _ = self.header.incr_write_offset(bytes);
        self.header.incr_live_bytes(bytes);
        self.header.incr_live_items();
    }

    pub(crate) fn alloc_item(&mut self, size: i32) -> RawItem {
        let offset = self.write_offset() as usize;
        self.incr_item(size);
        increment_gauge!(&Stat::ItemCurrent);
        increment_gauge_by!(&Stat::ItemCurrentBytes, size as i64);

        let ptr = unsafe { self.data.as_mut_ptr().add(offset) };
        RawItem::from_ptr(ptr)
    }

    pub(crate) fn remove_item(&mut self, item_info: u64, tombstone: bool) {
        let offset = get_offset(item_info) as usize;
        self.remove_item_at(offset, tombstone)
    }

    pub(crate) fn remove_item_at(&mut self, offset: usize, _tombstone: bool) {
        let mut item = self.get_item_at(offset).unwrap();
        if item.deleted() {
            return;
        }

        let item_size = item.size() as i64;

        decrement_gauge!(&Stat::ItemCurrent);
        decrement_gauge_by!(&Stat::ItemCurrentBytes, item_size);
        increment_gauge!(&Stat::ItemDead);
        increment_gauge_by!(&Stat::ItemDeadBytes, item_size);

        self.check_magic();
        self.decr_item(item_size as i32);
        assert!(self.live_bytes() >= 0);
        assert!(self.live_items() >= 0);
        item.tombstone();

        self.check_magic();
    }

    // returns the item looking it up from the item_info
    // TODO(bmartin): consider changing the return type here and removing asserts?
    pub(crate) fn get_item(&mut self, item_info: u64) -> Option<RawItem> {
        assert_eq!(get_seg_id(item_info) as i32, self.id());
        self.get_item_at(get_offset(item_info) as usize)
    }

    // returns the item at the given offset
    // TODO(bmartin): consider changing the return type here and removing asserts?
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn get_item_at(&mut self, offset: usize) -> Option<RawItem> {
        assert!(offset <= self.max_item_offset());
        Some(RawItem::from_ptr(unsafe {
            self.data.as_mut_ptr().add(offset)
        }))
    }

    // this is used as part of segment merging, it moves all occupied space to
    // the beginning of the segment, leaving the end of the segment free
    #[allow(clippy::unnecessary_wraps)]
    pub(crate) fn compact(&mut self, hashtable: &mut HashTable) -> Result<(), SegmentsError> {
        let max_offset = self.max_item_offset();
        let mut read_offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        let mut write_offset = read_offset;

        while read_offset <= max_offset {
            let item = self.get_item_at(read_offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            let item_size = item.size();

            // don't copy deleted items
            if item.deleted() {
                // since the segment won't be cleared, we decrement dead items
                decrement_gauge!(&Stat::ItemDead);
                decrement_gauge_by!(&Stat::ItemDeadBytes, item.size() as i64);
                // move read offset forward, leave write offset trailing
                read_offset += item_size;
                increment_counter!(&Stat::ItemCompacted);
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

        // updates the write offset to the new position
        self.set_write_offset(write_offset as i32);

        Ok(())
    }

    // this is used to copy data from this segment into the target segment and
    // relink the items in the hashtable
    // NOTE: any items that don't fit in the target will be left in this segment
    // it is left to the caller to decide how to handle this
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

        while read_offset <= max_offset {
            let item = self.get_item_at(read_offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }

            item.check_magic();

            let item_size = item.size();

            let write_offset = target.write_offset() as usize;

            // skip deleted items and ones that won't fit in the target segment
            if item.deleted() || write_offset + item_size >= target.data.len() {
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
                self.remove_item_at(read_offset, true);
                target.header.incr_live_items();
                target.header.incr_live_bytes(item_size as i32);
                target.set_write_offset(write_offset as i32 + item_size as i32);
                increment_gauge!(&Stat::ItemCurrent);
                increment_gauge_by!(&Stat::ItemCurrentBytes, item_size as i64);
            } else {
                // TODO(bmartin): figure out if this could happen and make the
                // relink function infallible if it can't happen
                return Err(SegmentsError::RelinkFailure);
            }

            read_offset += item_size;
        }

        Ok(())
    }

    // this is used as part of segment merging, it removes items from the
    // segment based on a cutoff frequency and target ratio. Since the cutoff
    // frequency is adjusted, it is returned in the result.
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

            if item.deleted() {
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
                    self.remove_item_at(offset, true);
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

            if !item.deleted() {
                items += 1;
                bytes += item.size();
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
                    self.remove_item_at(offset, true);
                }
            } else {
                items += 1;
                bytes += item.size();
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

        decrement_gauge_by!(&Stat::ItemDead, items as i64);
        decrement_gauge_by!(&Stat::ItemDeadBytes, bytes as i64);

        // skips over seg_wait_refcount and evict retry, because no threading

        if self.live_items() != 0 {
            assert_eq!(self.live_items(), 0, "segment not empty after clearing");
        }

        let expected_size = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC) as i32
        } else {
            0
        };
        if self.live_bytes() != expected_size {
            assert_eq!(
                self.live_bytes(),
                expected_size,
                "segment size incorrect after clearing"
            );
        }

        self.set_write_offset(self.live_bytes());
    }

    pub(crate) fn dump(&mut self) -> SegmentDump {
        let mut ret = SegmentDump {
            id: self.id(),
            write_offset: self.write_offset(),
            live_bytes: self.live_bytes(),
            live_items: self.live_items(),
            prev_seg: self.prev_seg().unwrap_or(-1),
            next_seg: self.next_seg().unwrap_or(-1),
            ttl: self.ttl().as_secs(),
            items: Vec::new(),
        };

        let max_offset = self.max_item_offset();
        let mut offset = if cfg!(feature = "magic") {
            std::mem::size_of_val(&SEG_MAGIC)
        } else {
            0
        };

        while offset <= max_offset {
            let item = self.get_item_at(offset).unwrap();
            if item.klen() == 0 && self.live_items() == 0 {
                break;
            }
            ret.items.push(ItemDump {
                offset: offset as i32,
                size: item.size() as i32,
                is_dead: item.deleted(),
            });

            offset += item.size();
        }
        ret
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SegmentDump {
    id: i32,
    write_offset: i32,
    live_bytes: i32,
    live_items: i32,
    prev_seg: i32,
    next_seg: i32,
    ttl: u32,
    items: Vec<ItemDump>,
}

#[derive(Serialize, Deserialize)]
pub struct ItemDump {
    offset: i32,
    size: i32,
    is_dead: bool,
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
