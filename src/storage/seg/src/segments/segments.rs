// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::eviction::*;
use crate::item::*;
use crate::segments::*;
use core::num::NonZeroU32;
use datapool::*;

/// `Segments` contain all items within the cache. This struct is a collection
/// of individual `Segment`s which are represented by a `SegmentHeader` and a
/// subslice of bytes from a contiguous heap allocation.
pub(crate) struct Segments {
    /// Pointer to slice of headers
    headers: Box<[SegmentHeader]>,
    /// Pointer to raw data
    data: Box<dyn Datapool>,
    /// Segment size in bytes
    segment_size: i32,
    /// Number of free segments
    free: u32,
    /// Total number of segments
    cap: u32,
    /// Head of the free segment queue
    free_q: Option<NonZeroU32>,
    /// Time last flushed
    flush_at: Instant,
    /// Eviction configuration and state
    evict: Box<Eviction>,
}

impl Segments {
    /// Private function which allocates and initializes the `Segments` by
    /// taking ownership of the builder
    pub(super) fn from_builder(builder: SegmentsBuilder) -> Result<Self, std::io::Error> {
        let segment_size = builder.segment_size;
        let segments = builder.heap_size / (builder.segment_size as usize);

        debug!(
            "heap size: {} seg size: {} segments: {}",
            builder.heap_size, segment_size, segments
        );

        assert!(
            segments < (1 << 24), // we use just 24 bits to store the seg id
            "heap size requires too many segments, reduce heap size or increase segment size"
        );

        let evict_policy = builder.evict_policy;

        debug!("eviction policy: {:?}", evict_policy);

        let mut headers = Vec::with_capacity(0);
        headers.reserve_exact(segments);
        for id in 0..segments {
            // safety: we start iterating from 1 and seg id is constrained to < 2^24
            let header = SegmentHeader::new(unsafe { NonZeroU32::new_unchecked(id as u32 + 1) });
            headers.push(header);
        }
        let mut headers = headers.into_boxed_slice();

        let heap_size = segments * segment_size as usize;

        // TODO(bmartin): we will need to make additional changes before we
        // allow restoring state from an existing datapool file, for now this
        // retains the previous behavior and always creates a new file to mmap
        // if a datapool path is provided.
        let mut data: Box<dyn Datapool> = if let Some(file) = builder.datapool_path {
            Box::new(MmapFile::create(file, heap_size, crate::VERSION)?)
        } else {
            Box::new(Memory::create(heap_size)?)
        };

        for idx in 0..segments {
            let begin = segment_size as usize * idx;
            let end = begin + segment_size as usize;

            let mut segment =
                Segment::from_raw_parts(&mut headers[idx], &mut data.as_mut_slice()[begin..end]);
            segment.init();

            let id = idx as u32 + 1; // we index segments from 1
            segment.set_prev_seg(NonZeroU32::new(id - 1));
            if id < segments as u32 {
                segment.set_next_seg(NonZeroU32::new(id + 1));
            }
        }

        SEGMENT_CURRENT.set(segments as _);
        SEGMENT_FREE.set(segments as _);

        Ok(Self {
            headers,
            segment_size,
            cap: segments as u32,
            free: segments as u32,
            free_q: NonZeroU32::new(1),
            data,
            flush_at: Instant::now(),
            evict: Box::new(Eviction::new(segments, evict_policy)),
        })
    }

    /// Return the size of each segment in bytes
    #[inline]
    pub fn segment_size(&self) -> i32 {
        self.segment_size
    }

    /// Returns the number of free segments
    #[cfg(test)]
    pub fn free(&self) -> usize {
        self.free as usize
    }

    /// Returns the time the segments were last flushed
    pub fn flush_at(&self) -> Instant {
        self.flush_at
    }

    /// Mark the segments as flushed at a given instant
    pub fn set_flush_at(&mut self, instant: Instant) {
        self.flush_at = instant;
    }

    /// Retrieve a `RawItem` from the segment id and offset encoded in the
    /// item info.
    pub(crate) fn get_item(&mut self, item_info: u64) -> Option<RawItem> {
        let seg_id = get_seg_id(item_info);
        let offset = get_offset(item_info) as usize;
        self.get_item_at(seg_id, offset)
    }

    /// Retrieve a `RawItem` from a specific segment id at the given offset
    // TODO(bmartin): consider changing the return type here and removing asserts?
    pub(crate) fn get_item_at(
        &mut self,
        seg_id: Option<NonZeroU32>,
        offset: usize,
    ) -> Option<RawItem> {
        let seg_id = seg_id.map(|v| v.get())?;
        trace!("getting item from: seg: {} offset: {}", seg_id, offset);
        assert!(seg_id <= self.cap as u32);

        let seg_begin = self.segment_size() as usize * (seg_id as usize - 1);
        let seg_end = seg_begin + self.segment_size() as usize;
        let mut segment = Segment::from_raw_parts(
            &mut self.headers[seg_id as usize - 1],
            &mut self.data.as_mut_slice()[seg_begin..seg_end],
        );

        segment.get_item_at(offset)
    }

    /// Tries to clear a segment by id
    fn clear_segment(
        &mut self,
        id: NonZeroU32,
        hashtable: &mut HashTable,
        expire: bool,
    ) -> Result<(), ()> {
        let mut segment = self.get_mut(id).unwrap();
        if segment.next_seg().is_none() && !expire {
            Err(())
        } else {
            // TODO(bmartin): this should probably result in an error and not be
            // an assert
            assert!(segment.evictable(), "segment was not evictable");
            segment.set_evictable(false);
            segment.set_accessible(false);
            segment.clear(hashtable, expire);
            Ok(())
        }
    }

    /// Perform eviction based on the configured eviction policy. A success from
    /// this function indicates that a segment was put onto the free queue and
    /// that `pop_free()` should return some segment id.
    pub fn evict(
        &mut self,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable,
    ) -> Result<(), SegmentsError> {
        let now = Instant::now();
        match self.evict.policy() {
            Policy::Merge { .. } => {
                SEGMENT_EVICT.increment();

                let mut seg_idx = self.evict.random();

                seg_idx %= self.cap;
                let ttl = self.headers[seg_idx as usize].ttl();
                let offset = ttl_buckets.get_bucket_index(ttl);
                let buckets = ttl_buckets.buckets.len();

                // since merging starts in the middle of a segment chain, we may
                // need to loop back around to the first ttl bucket we checked
                for i in 0..=buckets {
                    let bucket_id = (offset + i) % buckets;
                    let ttl_bucket = &mut ttl_buckets.buckets[bucket_id];
                    if let Some(first_seg) = ttl_bucket.head() {
                        let start = ttl_bucket.next_to_merge().unwrap_or(first_seg);
                        match self.merge_evict(start, hashtable) {
                            Ok(next_to_merge) => {
                                debug!("merged ttl_bucket: {} seg: {}", bucket_id, start);
                                ttl_bucket.set_next_to_merge(next_to_merge);
                                EVICT_TIME.add(now.elapsed().as_nanos() as _);
                                return Ok(());
                            }
                            Err(_) => {
                                SEGMENT_EVICT_EX.increment();
                                ttl_bucket.set_next_to_merge(None);
                                continue;
                            }
                        }
                    }
                }
                SEGMENT_EVICT_EX.increment();
                EVICT_TIME.add(now.elapsed().as_nanos() as _);
                Err(SegmentsError::NoEvictableSegments)
            }
            Policy::None => {
                EVICT_TIME.add(now.elapsed().as_nanos() as _);
                Err(SegmentsError::NoEvictableSegments)
            }
            _ => {
                SEGMENT_EVICT.increment();
                if let Some(id) = self.least_valuable_seg(ttl_buckets) {
                    let result = self
                        .clear_segment(id, hashtable, false)
                        .map_err(|_| SegmentsError::EvictFailure);

                    if result.is_err() {
                        EVICT_TIME.add(now.elapsed().as_nanos() as _);
                        return result;
                    }

                    let id_idx = id.get() as usize - 1;
                    if self.headers[id_idx].prev_seg().is_none() {
                        let ttl_bucket = ttl_buckets.get_mut_bucket(self.headers[id_idx].ttl());
                        ttl_bucket.set_head(self.headers[id_idx].next_seg());
                    }
                    self.push_free(id);
                    EVICT_TIME.add(now.elapsed().as_nanos() as _);
                    Ok(())
                } else {
                    SEGMENT_EVICT_EX.increment();
                    EVICT_TIME.add(now.elapsed().as_nanos() as _);
                    Err(SegmentsError::NoEvictableSegments)
                }
            }
        }
    }

    /// Returns a mutable `Segment` view for the segment with the specified id
    pub(crate) fn get_mut(&mut self, id: NonZeroU32) -> Result<Segment, SegmentsError> {
        let id = id.get() as usize - 1;
        if id < self.headers.len() {
            let header = self.headers.get_mut(id).unwrap();

            let seg_start = self.segment_size as usize * id;
            let seg_end = self.segment_size as usize * (id + 1);

            let seg_data = &mut self.data.as_mut_slice()[seg_start..seg_end];

            let segment = Segment::from_raw_parts(header, seg_data);
            segment.check_magic();
            Ok(segment)
        } else {
            Err(SegmentsError::BadSegmentId)
        }
    }

    /// Gets a mutable `Segment` view for two segments after making sure the
    /// borrows are disjoint.
    pub(crate) fn get_mut_pair(
        &mut self,
        a: NonZeroU32,
        b: NonZeroU32,
    ) -> Result<(Segment, Segment), SegmentsError> {
        if a == b {
            Err(SegmentsError::BadSegmentId)
        } else {
            let a = a.get() as usize - 1;
            let b = b.get() as usize - 1;
            if a >= self.headers.len() || b >= self.headers.len() {
                return Err(SegmentsError::BadSegmentId);
            }
            // we have already guaranteed that 'a' and 'b' are not the same, so
            // we know that they are disjoint borrows and can safely return
            // mutable borrows to both the segments
            unsafe {
                let seg_size = self.segment_size() as usize;

                let header_a = &mut self.headers[a] as *mut _;
                let header_b = &mut self.headers[b] as *mut _;

                let data = self.data.as_mut_slice();

                // split the borrowed data
                let split = (std::cmp::min(a, b) + 1) * seg_size;
                let (first, second) = data.split_at_mut(split);

                let (data_a, data_b) = if a < b {
                    let start_a = seg_size * a;
                    let end_a = seg_size * (a + 1);

                    let start_b = (seg_size * b) - first.len();
                    let end_b = (seg_size * (b + 1)) - first.len();

                    (&mut first[start_a..end_a], &mut second[start_b..end_b])
                } else {
                    let start_a = (seg_size * a) - first.len();
                    let end_a = (seg_size * (a + 1)) - first.len();

                    let start_b = seg_size * b;
                    let end_b = seg_size * (b + 1);

                    (&mut second[start_a..end_a], &mut first[start_b..end_b])
                };

                let segment_a = Segment::from_raw_parts(&mut *header_a, data_a);
                let segment_b = Segment::from_raw_parts(&mut *header_b, data_b);

                segment_a.check_magic();
                segment_b.check_magic();
                Ok((segment_a, segment_b))
            }
        }
    }

    /// Helper function which unlinks a segment from a chain by updating the
    /// pointers of previous and next segments.
    /// *NOTE*: this function must not be used on segments in the free queue
    fn unlink(&mut self, id: NonZeroU32) {
        let id_idx = id.get() as usize - 1;

        if let Some(next) = self.headers[id_idx].next_seg() {
            let prev = self.headers[id_idx].prev_seg();
            self.headers[next.get() as usize - 1].set_prev_seg(prev);
        }

        if let Some(prev) = self.headers[id_idx].prev_seg() {
            let next = self.headers[id_idx].next_seg();
            self.headers[prev.get() as usize - 1].set_next_seg(next);
        }
    }

    /// Helper function which pushes a segment onto the front of a chain.
    fn push_front(&mut self, this: NonZeroU32, head: Option<NonZeroU32>) {
        let this_idx = this.get() as usize - 1;
        self.headers[this_idx].set_next_seg(head);
        self.headers[this_idx].set_prev_seg(None);

        if let Some(head_id) = head {
            let head_idx = head_id.get() as usize - 1;
            debug_assert!(self.headers[head_idx].prev_seg().is_none());
            self.headers[head_idx].set_prev_seg(Some(this));
        }
    }

    /// Returns a segment to the free queue, to be used after clearing the
    /// segment.
    pub(crate) fn push_free(&mut self, id: NonZeroU32) {
        SEGMENT_RETURN.increment();
        SEGMENT_FREE.increment();
        // unlinks the next segment
        self.unlink(id);

        // relinks it as the free queue head
        self.push_front(id, self.free_q);
        self.free_q = Some(id);

        let id_idx = id.get() as usize - 1;
        assert!(!self.headers[id_idx].evictable());
        self.headers[id_idx].set_accessible(false);

        self.headers[id_idx].reset();

        self.free += 1;
    }

    /// Try to take a segment from the free queue. Returns the segment id which
    /// must then be linked into a segment chain.
    pub(crate) fn pop_free(&mut self) -> Option<NonZeroU32> {
        assert!(self.free <= self.cap);

        if self.free == 0 {
            None
        } else {
            SEGMENT_REQUEST.increment();
            SEGMENT_REQUEST_SUCCESS.increment();
            SEGMENT_FREE.decrement();
            self.free -= 1;
            let id = self.free_q;
            assert!(id.is_some());

            let id_idx = id.unwrap().get() as usize - 1;

            if let Some(next) = self.headers[id_idx].next_seg() {
                self.free_q = Some(next);
                // this is not really necessary
                let next = &mut self.headers[next.get() as usize - 1];
                next.set_prev_seg(None);
            } else {
                self.free_q = None;
            }

            #[cfg(not(feature = "magic"))]
            assert_eq!(self.headers[id_idx].write_offset(), 0);

            #[cfg(feature = "magic")]
            assert_eq!(
                self.headers[id_idx].write_offset() as usize,
                std::mem::size_of_val(&SEG_MAGIC),
                "segment: ({}) in free queue has write_offset: ({})",
                id.unwrap(),
                self.headers[id_idx].write_offset()
            );

            common::time::refresh_clock();
            self.headers[id_idx].mark_created();
            self.headers[id_idx].mark_merged();

            id
        }
    }

    // TODO(bmartin): use a result here, not option
    /// Returns the least valuable segment based on the configured eviction
    /// policy. An eviction attempt should be made for the corresponding segment
    /// before moving on to the next least valuable segment.
    pub(crate) fn least_valuable_seg(
        &mut self,
        ttl_buckets: &mut TtlBuckets,
    ) -> Option<NonZeroU32> {
        match self.evict.policy() {
            Policy::None => None,
            Policy::Random => {
                let mut start: u32 = self.evict.random();

                start %= self.cap;

                for i in 0..self.cap {
                    let idx = (start + i) % self.cap;
                    if self.headers[idx as usize].can_evict() {
                        // safety: we are always adding 1 to the index
                        return Some(unsafe { NonZeroU32::new_unchecked(idx + 1) });
                    }
                }

                None
            }
            Policy::RandomFifo => {
                // This strategy is implemented by picking a random accessible
                // segment and looking up the head of the corresponding
                // `TtlBucket` and evicting that segment. This is functionally
                // equivalent to picking a `TtlBucket` from a weighted
                // distribution based on the number of segments per bucket.

                let mut start: u32 = self.evict.random();

                start %= self.cap;

                for i in 0..self.cap {
                    let idx = (start + i) % self.cap;
                    if self.headers[idx as usize].accessible() {
                        let ttl = self.headers[idx as usize].ttl();
                        let ttl_bucket = ttl_buckets.get_mut_bucket(ttl);
                        return ttl_bucket.head();
                    }
                }

                None
            }
            _ => {
                if self.evict.should_rerank() {
                    self.evict.rerank(&self.headers);
                }
                while let Some(id) = self.evict.least_valuable_seg() {
                    if let Ok(seg) = self.get_mut(id) {
                        if seg.can_evict() {
                            return Some(id);
                        }
                    }
                }
                None
            }
        }
    }

    /// Remove a single item from a segment based on the item_info
    pub(crate) fn remove_item(
        &mut self,
        item_info: u64,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable,
    ) -> Result<(), SegmentsError> {
        if let Some(seg_id) = get_seg_id(item_info) {
            let offset = get_offset(item_info) as usize;
            self.remove_at(seg_id, offset, ttl_buckets, hashtable)
        } else {
            Err(SegmentsError::BadSegmentId)
        }
    }

    /// Remove a single item from a segment based on the segment id and offset.
    pub(crate) fn remove_at(
        &mut self,
        seg_id: NonZeroU32,
        offset: usize,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable,
    ) -> Result<(), SegmentsError> {
        // remove the item
        {
            let mut segment = self.get_mut(seg_id)?;
            segment.remove_item_at(offset);

            // regardless of eviction policy, we can evict the segment if its now
            // empty and would be evictable. if we evict, we must return early
            if segment.live_items() == 0 && segment.can_evict() {
                // even though the item has zero live items, we clear it as a
                // way of updating the dead item metrics.
                segment.clear(hashtable, false);

                segment.set_evictable(false);
                // if it's the head of a ttl bucket, we need to manually relink
                // the bucket head while we have access to the ttl buckets
                if segment.prev_seg().is_none() {
                    let ttl_bucket = ttl_buckets.get_mut_bucket(segment.ttl());
                    ttl_bucket.set_head(segment.next_seg());
                }
                self.push_free(seg_id);
                return Ok(());
            }
        }

        // for merge eviction, we check if the segment is now below the target
        // ratio which serves as a low watermark for occupancy. if it is, we do
        // a no-evict merge (compaction only, no-pruning)
        if let Policy::Merge { .. } = self.evict.policy() {
            let target_ratio = self.evict.compact_ratio();

            let id_idx = seg_id.get() as usize - 1;

            let ratio = self.headers[id_idx].live_bytes() as f64 / self.segment_size() as f64;

            // if this segment occupancy is higher than the target ratio, skip
            // merge
            if ratio > target_ratio {
                return Ok(());
            }

            if let Some(next_id) = self.headers[id_idx].next_seg() {
                // require that this segment has not merged recently, this
                // reduces CPU load under heavy rewrite/delete workloads at the
                // cost of letting more dead items remain in the segements,
                // reducing the hitrate
                // if self.headers[seg_id as usize].merge_at() + CoarseDuration::from_secs(30) > CoarseInstant::recent() {
                //     return Ok(());
                // }

                let next_idx = next_id.get() as usize - 1;

                // if the next segment can't be evicted, we shouldn't merge
                if !self.headers[next_idx].can_evict() {
                    return Ok(());
                }

                // calculate occupancy ratio of the next segment
                let next_ratio =
                    self.headers[next_idx].live_bytes() as f64 / self.segment_size() as f64;

                // if the next segment is empty enough, proceed to merge compaction
                if next_ratio <= target_ratio {
                    let _ = self.merge_compact(seg_id, hashtable);
                    // we need to make sure the ttl bucket doesn't have a pointer to
                    // any of the segments we removed through merging.
                    let ttl_bucket = ttl_buckets.get_mut_bucket(self.headers[id_idx].ttl());
                    ttl_bucket.set_next_to_merge(None);
                }
            }
        }

        Ok(())
    }

    // mostly for testing, probably never want to run this otherwise
    #[cfg(any(test, feature = "debug"))]
    pub(crate) fn items(&mut self) -> usize {
        let mut total = 0;
        for id in 1..=self.cap {
            // this is safe because we start iterating from 1
            let segment = self
                .get_mut(unsafe { NonZeroU32::new_unchecked(id as u32) })
                .unwrap();
            segment.check_magic();
            let count = segment.live_items();
            debug!("{} items in segment {} segment: {:?}", count, id, segment);
            total += segment.live_items() as usize;
        }
        total
    }

    #[cfg(test)]
    pub(crate) fn print_headers(&self) {
        for id in 0..self.cap {
            println!("segment header: {:?}", self.headers[id as usize]);
        }
    }

    #[cfg(feature = "debug")]
    pub(crate) fn check_integrity(&mut self, hashtable: &mut HashTable) -> bool {
        let mut integrity = true;
        for id in 0..self.cap {
            if !self
                .get_mut(NonZeroU32::new(id + 1).unwrap())
                .unwrap()
                .check_integrity(hashtable)
            {
                integrity = false;
            }
        }
        integrity
    }

    fn merge_evict_chain_len(&mut self, start: NonZeroU32) -> usize {
        let mut len = 0;
        let mut id = start;
        let max = self.evict.max_merge();

        while len < max {
            if let Ok(seg) = self.get_mut(id) {
                if seg.can_evict() {
                    len += 1;
                    match seg.next_seg() {
                        Some(i) => {
                            id = i;
                        }
                        None => {
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                warn!("invalid segment id: {}", id);
                break;
            }
        }

        len
    }

    fn merge_compact_chain_len(&mut self, start: NonZeroU32) -> usize {
        let mut len = 0;
        let mut id = start;
        let max = self.evict.max_merge();
        let mut occupied = 0;
        let seg_size = self.segment_size();

        while len < max {
            if let Ok(seg) = self.get_mut(id) {
                if seg.can_evict() {
                    occupied += seg.live_bytes();
                    if occupied > seg_size {
                        break;
                    }
                    len += 1;
                    match seg.next_seg() {
                        Some(i) => {
                            id = i;
                        }
                        None => {
                            break;
                        }
                    }
                } else {
                    break;
                }
            } else {
                warn!("invalid segment id: {}", id);
                break;
            }
        }

        len
    }

    fn merge_evict(
        &mut self,
        start: NonZeroU32,
        hashtable: &mut HashTable,
    ) -> Result<Option<NonZeroU32>, SegmentsError> {
        SEGMENT_MERGE.increment();

        let dst_id = start;
        let chain_len = self.merge_evict_chain_len(start);

        // TODO(bmartin): this should be a different error probably
        if chain_len < 3 {
            return Err(SegmentsError::NoEvictableSegments);
        }

        let mut next_id = self.get_mut(start).map(|s| s.next_seg())?;

        // merge state
        let mut cutoff = 1.0;
        let mut merged = 0;

        // fixed merge parameters
        let max_merge = self.evict.max_merge();
        let n_merge = self.evict.n_merge();
        let stop_ratio = self.evict.stop_ratio();
        let stop_bytes = (stop_ratio * self.segment_size() as f64) as i32;

        // dynamically set the target ratio based on the length of the merge chain
        let target_ratio = if chain_len < n_merge {
            1.0 / chain_len as f64
        } else {
            self.evict.target_ratio()
        };

        // prune and compact target segment
        {
            let mut dst = self.get_mut(start)?;
            let dst_old_size = dst.live_bytes();

            trace!("prune merge with cutoff: {}", cutoff);
            cutoff = dst.prune(hashtable, cutoff, target_ratio);
            trace!("cutoff is now: {}", cutoff);

            dst.compact(hashtable)?;

            let dst_new_size = dst.live_bytes();
            trace!(
                "dst {}: {} bytes -> {} bytes",
                dst_id,
                dst_old_size,
                dst_new_size
            );

            dst.mark_merged();
            merged += 1;
        }

        // while we still want to merge and can, we prune and compact the source
        // and then copy into the destination. If the destination becomes full,
        // we stop merging
        while let Some(src_id) = next_id {
            if merged > max_merge {
                trace!("stop merge: merged max segments");
                break;
            }

            if !self.get_mut(src_id).map(|s| s.can_evict()).unwrap_or(false) {
                trace!("stop merge: can't evict source segment");
                return Ok(None); // this causes the next_to_merge to reset
            }

            let (mut dst, mut src) = self.get_mut_pair(dst_id, src_id)?;

            let dst_start_size = dst.live_bytes();
            let src_start_size = src.live_bytes();

            if dst_start_size >= stop_bytes {
                trace!("stop merge: target segment is full");
                break;
            }

            trace!("pruning source segment");
            cutoff = src.prune(hashtable, cutoff, target_ratio);

            trace!(
                "src {}: {} bytes -> {} bytes",
                src_id,
                src_start_size,
                src.live_bytes()
            );

            trace!("copying source into target");
            let _ = src.copy_into(&mut dst, hashtable);
            trace!("copy dropped {} bytes", src.live_bytes());

            trace!(
                "dst {}: {} bytes -> {} bytes",
                dst_id,
                dst_start_size,
                dst.live_bytes()
            );

            next_id = src.next_seg();
            src.clear(hashtable, false);
            self.push_free(src_id);
            merged += 1;
        }

        Ok(next_id)
    }

    fn merge_compact(
        &mut self,
        start: NonZeroU32,
        hashtable: &mut HashTable,
    ) -> Result<Option<NonZeroU32>, SegmentsError> {
        SEGMENT_MERGE.increment();

        let dst_id = start;

        let chain_len = self.merge_compact_chain_len(start);

        // TODO(bmartin): this should be a different error probably
        if chain_len < 2 {
            return Err(SegmentsError::NoEvictableSegments);
        }

        let mut next_id = self.get_mut(start).map(|s| s.next_seg())?;

        // TODO(bmartin): this should be a different error probably
        // TODO(bmartin): maybe not needed with the merge chain len check above
        if next_id.is_none() {
            return Err(SegmentsError::NoEvictableSegments);
        }

        // merge state
        let mut merged = 0;

        // fixed merge parameters
        let seg_size = self.segment_size();
        let max_merge = self.evict.max_merge();
        let stop_ratio = self.evict.stop_ratio();
        let stop_bytes = (stop_ratio * self.segment_size() as f64) as i32;

        // prune and compact target segment
        {
            let mut dst = self.get_mut(start)?;
            let dst_old_size = dst.live_bytes();

            dst.compact(hashtable)?;

            let dst_new_size = dst.live_bytes();
            trace!(
                "dst {}: {} bytes -> {} bytes",
                dst_id,
                dst_old_size,
                dst_new_size
            );

            dst.mark_merged();
            merged += 1;
        }

        // while we still want to merge and can, we prune and compact the source
        // and then copy into the destination. If the destination becomes full,
        // we stop merging
        while let Some(src_id) = next_id {
            if merged > max_merge {
                trace!("stop merge: merged max segments");
                break;
            }

            if !self.get_mut(src_id).map(|s| s.can_evict()).unwrap_or(false) {
                trace!("stop merge: can't evict source segment");
                return Ok(None); // this causes the next_to_merge to reset
            }

            let (mut dst, mut src) = self.get_mut_pair(dst_id, src_id)?;

            let dst_start_size = dst.live_bytes();
            let src_start_size = src.live_bytes();

            if dst_start_size >= stop_bytes {
                trace!("stop merge: target segment is full");
                break;
            }

            if dst_start_size + src_start_size > seg_size {
                break;
            }

            trace!(
                "src {}: {} bytes -> {} bytes",
                src_id,
                src_start_size,
                src.live_bytes()
            );

            trace!("copying source into target");
            let _ = src.copy_into(&mut dst, hashtable);
            trace!("copy dropped {} bytes", src.live_bytes());

            trace!(
                "dst {}: {} bytes -> {} bytes",
                dst_id,
                dst_start_size,
                dst.live_bytes()
            );

            next_id = src.next_seg();
            src.clear(hashtable, false);
            self.push_free(src_id);
            merged += 1;
        }

        Ok(next_id)
    }
}
