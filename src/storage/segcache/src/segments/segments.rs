use crate::segments::segment::SegmentDump;
use crate::segments::*;
use thiserror::Error;

use metrics::Stat;
use rustcommon_time::CoarseInstant as Instant;

#[derive(Error, Debug)]
pub enum SegmentsError {
    #[error("bad segment id")]
    BadSegmentId,
    #[error("item relink failure")]
    RelinkFailure,
    #[error("no evictable segments")]
    NoEvictableSegments,
    #[error("evict failure")]
    EvictFailure,
}

pub struct SegmentsBuilder {
    heap_size: usize,
    segment_size: i32,
    evict_policy: Policy,
}

impl Default for SegmentsBuilder {
    fn default() -> Self {
        Self {
            segment_size: 1024 * 1024,
            heap_size: 64 * 1024 * 1024,
            evict_policy: Policy::Random,
        }
    }
}

impl<'a> SegmentsBuilder {
    pub fn segment_size(mut self, bytes: i32) -> Self {
        #[cfg(not(feature = "magic"))]
        assert!(bytes > ITEM_HDR_SIZE as i32);

        #[cfg(feature = "magic")]
        assert!(bytes >= ITEM_HDR_SIZE as i32 + ITEM_MAGIC_SIZE as i32);

        self.segment_size = bytes;
        self
    }

    pub fn heap_size(mut self, bytes: usize) -> Self {
        self.heap_size = bytes;
        self
    }

    pub fn eviction_policy(mut self, policy: Policy) -> Self {
        self.evict_policy = policy;
        self
    }

    pub fn build(self) -> Segments {
        Segments::from_builder(self)
    }
}

pub struct Segments {
    headers: Box<[SegmentHeader]>, // pointer to slice of headers
    data: Box<[u8]>,               // pointer to raw data
    segment_size: i32,             // size of segment in bytes
    free: i32,                     // number of segments free
    cap: i32,                      // total number of segments
    free_q: i32,                   // next free segment
    flush_at: CoarseInstant,       // time last flushed
    evict: Box<Eviction>,          // eviction config and state
}

impl Default for Segments {
    fn default() -> Self {
        Self::from_builder(Default::default())
    }
}

impl Segments {
    fn from_builder(builder: SegmentsBuilder) -> Self {
        let segment_size = builder.segment_size;
        let segments = builder.heap_size / (builder.segment_size as usize);

        assert!(
            segments < (i32::MAX as usize),
            "heap size requires too many segments, reduce heap size or increase segment size"
        );

        let evict_policy = builder.evict_policy;

        let mut headers = Vec::with_capacity(0);
        headers.reserve_exact(segments);
        for id in 0..segments {
            let header = SegmentHeader::new(id as i32);
            headers.push(header);
        }
        let mut headers = headers.into_boxed_slice();

        let heap_size = segments * segment_size as usize;
        let mut data = Vec::with_capacity(0);
        data.reserve_exact(heap_size);
        data.resize(heap_size, 0);
        let mut data = data.into_boxed_slice();

        for id in 0..segments {
            let begin = segment_size as usize * id;
            let end = begin + segment_size as usize;

            let mut segment = Segment::from_raw_parts(&mut headers[id], &mut data[begin..end]);
            segment.init();
            if id > 0 {
                segment.set_prev_seg(id as i32 - 1);
            }
            if id < (segments - 1) {
                segment.set_next_seg(id as i32 + 1);
            }
        }

        increment_gauge_by!(&Stat::SegmentCurrent, segments as i64);
        increment_gauge_by!(&Stat::SegmentFree, segments as i64);

        Self {
            headers,
            segment_size,
            cap: segments as i32,
            free: segments as i32,
            free_q: 0,
            data,
            flush_at: Instant::recent(),
            evict: Box::new(Eviction::new(segments, evict_policy)),
        }
    }

    // returns the segment size in bytes
    #[inline]
    pub fn segment_size(&self) -> i32 {
        self.segment_size
    }

    // returns the number of segments in the free queue
    #[cfg(test)]
    pub fn free(&self) -> usize {
        self.free as usize
    }

    pub fn flush_at(&self) -> CoarseInstant {
        self.flush_at
    }

    pub(crate) fn get_item(&mut self, item_info: u64) -> Option<RawItem> {
        let seg_id = get_seg_id(item_info);
        let offset = get_offset(item_info) as usize;
        self.get_item_at(seg_id, offset)
    }

    // returns the item looking it up from the item_info
    // TODO(bmartin): consider changing the return type here and removing asserts?
    pub(crate) fn get_item_at(&mut self, seg_id: i32, offset: usize) -> Option<RawItem> {
        trace!("getting item from: seg: {} offset: {}", seg_id, offset);
        assert!(seg_id < self.cap as i32);

        let seg_begin = self.segment_size() as usize * seg_id as usize;
        let seg_end = seg_begin + self.segment_size() as usize;
        let mut segment = Segment::from_raw_parts(&mut self.headers[seg_id as usize], &mut self.data[seg_begin..seg_end]);

        segment.get_item_at(offset)
    }

    fn clear_segment<S: BuildHasher>(
        &mut self,
        id: i32,
        hashtable: &mut HashTable<S>,
        expire: bool,
    ) -> Result<(), ()> {
        let mut segment = self.get_mut(id).unwrap();
        if segment.next_seg().is_none() && !expire {
            Err(())
        } else {
            assert_eq!(segment.evictable(), true);
            segment.set_evictable(false);
            segment.set_accessible(false);
            segment.clear(hashtable, expire);
            Ok(())
        }
    }

    pub fn evict<S: BuildHasher>(
        &mut self,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable<S>,
    ) -> Result<(), SegmentsError> {
        match self.evict.policy() {
            Policy::Merge { .. } => {
                increment_counter!(&Stat::SegmentEvict);

                let mut seg_id: i32 = self.evict.random();
                if seg_id < 0 {
                    seg_id *= -1;
                }

                seg_id %= self.cap;
                let ttl = self.headers[seg_id as usize].ttl();
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
                                return Ok(());
                            }
                            Err(_) => {
                                increment_counter!(&Stat::SegmentEvictEx);
                                ttl_bucket.set_next_to_merge(None);
                                continue;
                            }
                        }
                    }
                }
                increment_counter!(&Stat::SegmentEvictEx);
                Err(SegmentsError::NoEvictableSegments)
            }
            Policy::None => Err(SegmentsError::NoEvictableSegments),
            _ => {
                increment_counter!(&Stat::SegmentEvict);
                if let Some(id) = self.least_valuable_seg() {
                    self.clear_segment(id, hashtable, false)
                        .map_err(|_| SegmentsError::EvictFailure)?;
                    if self.headers[id as usize].prev_seg().is_none() {
                        let ttl_bucket =
                            ttl_buckets.get_mut_bucket(self.headers[id as usize].ttl());
                        ttl_bucket.set_head(self.headers[id as usize].next_seg());
                    }
                    self.push_free(id);
                    Ok(())
                } else {
                    increment_counter!(&Stat::SegmentEvictEx);
                    Err(SegmentsError::NoEvictableSegments)
                }
            }
        }
    }

    // looks up the segment by id and returns a mutable view of it
    pub(crate) fn get_mut(&mut self, id: i32) -> Result<Segment, SegmentsError> {
        if id < 0 {
            Err(SegmentsError::BadSegmentId)
        } else {
            let id = id as usize;
            let header = self
                .headers
                .get_mut(id)
                .ok_or(SegmentsError::BadSegmentId)?;

            // this is safe because we now know the id was within range
            let data_ptr = unsafe { self.data.as_mut_ptr().add(self.segment_size as usize * id) };
            let data = unsafe { std::slice::from_raw_parts_mut(data_ptr, self.segment_size as usize) };

            let segment = Segment::from_raw_parts(header, data);
            segment.check_magic();
            Ok(segment)
        }
    }

    // gets a pair of mutable segments
    pub(crate) fn get_mut_pair(
        &mut self,
        a: i32,
        b: i32,
    ) -> Result<(Segment, Segment), SegmentsError> {
        if a < 0 || b < 0 || a == b {
            Err(SegmentsError::BadSegmentId)
        } else {
            let a = a as usize;
            let b = b as usize;
            if a > self.headers.len() || b > self.headers.len() {
                return Err(SegmentsError::BadSegmentId);
            }
            // we have already guaranteed that 'a' and 'b' are not the same, so
            // we know that they are disjoint borrows and can safely return
            // mutable borrows to both the segments
            unsafe {
                let header_a = &mut self.headers[a] as *mut _;
                let header_b = &mut self.headers[b] as *mut _;
                let data_ptr_a = self.data.as_mut_ptr().add(self.segment_size() as usize * a);
                let data_ptr_b = self.data.as_mut_ptr().add(self.segment_size() as usize * b);
                let data_a = std::slice::from_raw_parts_mut(data_ptr_a, self.segment_size() as usize);
                let data_b = std::slice::from_raw_parts_mut(data_ptr_b, self.segment_size() as usize);

                let segment_a = Segment::from_raw_parts(&mut *header_a, data_a);
                let segment_b = Segment::from_raw_parts(&mut *header_b, data_b);

                segment_a.check_magic();
                segment_b.check_magic();
                Ok((segment_a, segment_b))
            }
        }
    }

    fn unlink(&mut self, id: i32) {
        let id_idx = id as usize;
        if let Some(next) = self.headers[id_idx].next_seg() {
            let prev = self.headers[id_idx].prev_seg().unwrap_or(-1);
            self.headers[next as usize].set_prev_seg(prev);
        }

        if let Some(prev) = self.headers[id_idx].prev_seg() {
            let next = self.headers[id_idx].next_seg().unwrap_or(-1);
            self.headers[prev as usize].set_next_seg(next);
        }
    }

    fn push_front(&mut self, this: i32, head: i32) {
        let this_idx = this as usize;
        self.headers[this_idx].set_next_seg(head);
        self.headers[this_idx].set_prev_seg(-1);

        if head.is_some() {
            let head_idx = head as usize;
            debug_assert!(self.headers[head_idx].prev_seg().is_none());
            self.headers[head_idx].set_prev_seg(this);
        }
    }

    // adds a segment to the free queue
    pub(crate) fn push_free(&mut self, id: i32) {
        increment_counter!(&Stat::SegmentReturn);
        increment_gauge!(&Stat::SegmentFree);
        // unlinks the next segment
        self.unlink(id);

        // relinks it as the free queue head
        self.push_front(id, self.free_q);
        self.free_q = id;

        assert!(!self.headers[id as usize].evictable());
        self.headers[id as usize].set_accessible(false);

        self.headers[id as usize].reset();

        self.free += 1;
    }

    // get a segment from the free queue
    pub(crate) fn pop_free(&mut self) -> Option<i32> {
        assert!(self.free >= 0);
        assert!(self.free <= self.cap);

        increment_counter!(&Stat::SegmentRequest);

        if self.free == 0 {
            increment_counter!(&Stat::SegmentRequestEx);
            None
        } else {
            decrement_gauge!(&Stat::SegmentFree);
            self.free -= 1;
            let id = self.free_q;
            assert!(id.is_some());

            if let Some(next) = self.headers[id as usize].next_seg() {
                self.free_q = next;
                // this is not really necessary
                let next = &mut self.headers[next as usize];
                next.set_prev_seg(-1);
            } else {
                self.free_q = -1;
            }

            #[cfg(not(feature = "magic"))]
            assert_eq!(self.headers[id as usize].write_offset(), 0);

            #[cfg(feature = "magic")]
            assert_eq!(
                self.headers[id as usize].write_offset() as usize,
                std::mem::size_of_val(&SEG_MAGIC),
                "segment: ({}) in free queue has write_offset: ({})",
                id,
                self.headers[id as usize].write_offset()
            );

            rustcommon_time::refresh_clock();
            self.headers[id as usize].mark_created();
            self.headers[id as usize].mark_merged();

            Some(id)
        }
    }

    // TODO(bmartin): use a result here, not option
    pub(crate) fn least_valuable_seg(&mut self) -> Option<i32> {
        match self.evict.policy() {
            Policy::None => None,
            Policy::Random => {
                let mut start: i32 = self.evict.random();
                if start < 0 {
                    start *= -1;
                }

                start %= self.cap;

                for i in 0..self.cap {
                    let id = (start + i) % self.cap;
                    if self.headers[id as usize].can_evict() {
                        return Some(id);
                    }
                }

                None
            }
            _ => {
                if self.evict.should_rerank() {
                    self.evict.rerank(&self.headers);
                }
                while let Some(id) = self.evict.least_valuable_seg() {
                    if self.headers[id as usize].can_evict() {
                        return Some(id);
                    }
                }
                None
            }
        }
    }

    // remove a single item from a segment based on the item_info, optionally
    // setting tombstone
    pub(crate) fn remove_item<S: BuildHasher>(
        &mut self,
        item_info: u64,
        tombstone: bool,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable<S>,
    ) -> Result<(), SegmentsError> {
        let seg_id = get_seg_id(item_info);
        let offset = get_offset(item_info) as usize;
        self.remove_at(seg_id, offset, tombstone, ttl_buckets, hashtable)
    }

    pub(crate) fn remove_at<S: BuildHasher>(
        &mut self,
        seg_id: i32,
        offset: usize,
        tombstone: bool,
        ttl_buckets: &mut TtlBuckets,
        hashtable: &mut HashTable<S>,
    ) -> Result<(), SegmentsError> {
        // remove the item
        {
            let mut segment = self.get_mut(seg_id)?;
            segment.remove_item_at(offset, tombstone);
        }

        // regardless of eviction policy, we can evict the segment if its now
        // empty and would be evictable. if we evict, we must return early
        if self.headers[seg_id as usize].live_items() == 0 && self.headers[seg_id as usize].can_evict()
        {
            // NOTE: we skip clearing because we know the segment is empty
            self.headers[seg_id as usize].set_evictable(false);
            if self.headers[seg_id as usize].prev_seg().is_none() {
                let ttl_bucket = ttl_buckets.get_mut_bucket(self.headers[seg_id as usize].ttl());
                ttl_bucket.set_head(self.headers[seg_id as usize].next_seg());
            }
            self.push_free(seg_id);
            return Ok(());
        }

        // for merge eviction, we check if the segment is now below the target
        // ratio which serves as a low watermark for occupancy. if it is, we do
        // a no-evict merge (compaction only, no-pruning)
        if let Policy::Merge { .. } = self.evict.policy() {
            let target_ratio = self.evict.compact_ratio();

            let ratio =
                self.headers[seg_id as usize].live_bytes() as f64 / self.segment_size() as f64;

            // if this segment occupancy is higher than the target ratio, skip
            // merge
            if ratio > target_ratio {
                return Ok(());
            }

            if let Some(next_id) = self.headers[seg_id as usize].next_seg() {
                // require that this segment has not merged recently, this
                // reduces CPU load under heavy rewrite/delete workloads at the
                // cost of letting more dead items remain in the segements,
                // reducing the hitrate
                // if self.headers[seg_id as usize].merge_at() + CoarseDuration::from_secs(30) > CoarseInstant::recent() {
                //     return Ok(());
                // }

                // if the next segment can't be evicted, we shouldn't merge
                if !self.headers[next_id as usize].can_evict() {
                    return Ok(());
                }

                // calculate occupancy ratio of the next segment
                let next_ratio = self.headers[next_id as usize].live_bytes() as f64
                    / self.segment_size() as f64;

                // if the next segment is empty enough, proceed to merge compaction
                if next_ratio <= target_ratio {
                    let _ = self.merge_compact(seg_id, hashtable);
                    // we need to make sure the ttl bucket doesn't have a pointer to
                    // any of the segments we removed through merging.
                    let ttl_bucket =
                        ttl_buckets.get_mut_bucket(self.headers[seg_id as usize].ttl());
                    ttl_bucket.set_next_to_merge(None);
                }
            }
        }

        Ok(())
    }

    // mostly for testing, probably never want to run this otherwise
    pub(crate) fn items(&mut self) -> usize {
        let mut total = 0;
        for id in 0..self.cap {
            let segment = self.get_mut(id as i32).unwrap();
            segment.check_magic();
            let count = segment.live_items();
            debug!(
                "{} items in segment {} segment: {:?}",
                count, id, segment
            );
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

    pub(crate) fn check_integrity(&mut self) -> bool {
        let mut integrity = true;
        for id in 0..self.cap {
            if !self.get_mut(id).unwrap().check_integrity() {
                integrity = false;
            }
        }
        integrity
    }

    pub(crate) fn dump(&mut self) -> Vec<SegmentDump> {
        let mut ret = Vec::new();
        for id in 0..self.cap {
            let mut segment = self.get_mut(id).unwrap();
            ret.push(segment.dump());
        }
        ret
    }

    fn merge_evict_chain_len(&mut self, start: i32) -> usize {
        let mut len = 0;
        let mut start = start.as_option();
        let max = self.evict.max_merge();

        if start.is_some() {
            while len < max {
                if self.headers[start.unwrap() as usize].can_evict() {
                    len += 1;
                    start = self.headers[start.unwrap() as usize].next_seg();
                } else {
                    break;
                }
            }
        }

        len
    }

    fn merge_compact_chain_len(&mut self, start: i32) -> usize {
        let mut len = 0;
        let mut start = start.as_option();
        let max = self.evict.max_merge();
        let mut occupied = 0;
        let seg_size = self.segment_size();

        if start.is_some() {
            while len < max {
                if let Ok(seg) = self.get_mut(start.unwrap()) {
                    if seg.can_evict() {
                        occupied += seg.live_bytes();
                        if occupied > seg_size {
                            break;
                        }
                        len += 1;
                        start = seg.next_seg();
                    } else {
                        break;
                    }
                } else {
                    warn!("invalid segment id: {}", start.unwrap());
                    break;
                }
            }
        }

        len
    }

    fn merge_evict<S: BuildHasher>(
        &mut self,
        start: i32,
        hashtable: &mut HashTable<S>,
    ) -> Result<Option<i32>, SegmentsError> {
        increment_counter!(&Stat::SegmentMerge);

        let dst_id = start;
        let chain_len = self.merge_evict_chain_len(start);

        // TODO(bmartin): this should be a different error probably
        if chain_len < 3 {
            return Err(SegmentsError::NoEvictableSegments);
        }

        let mut next_id = self.headers[start as usize].next_seg();

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

            if !self.headers[src_id as usize].can_evict() {
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

    fn merge_compact<S: BuildHasher>(
        &mut self,
        start: i32,
        hashtable: &mut HashTable<S>,
    ) -> Result<Option<i32>, SegmentsError> {
        increment_counter!(&Stat::SegmentMerge);

        let dst_id = start;

        let chain_len = self.merge_compact_chain_len(start);

        // TODO(bmartin): this should be a different error probably
        if chain_len < 2 {
            return Err(SegmentsError::NoEvictableSegments);
        }

        let mut next_id = self.headers[start as usize].next_seg();

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

            if !self.headers[src_id as usize].can_evict() {
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
