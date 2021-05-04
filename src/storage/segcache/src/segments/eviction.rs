// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::segments::*;
use core::cmp::max;
use core::cmp::Ordering;
use core::num::NonZeroU32;

use rustcommon_time::CoarseInstant as Instant;

/// Policies define the eviction strategy to be used. All eviction strategies
/// exclude segments which are currently accepting new items.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// No eviction. When all the segments are full, inserts will fail until
    /// segments are freed by TTL expiration.
    None,
    /// Segment random eviction. Selects a random segment and evicts it. Similar
    /// to slab random eviction.
    Random,
    /// FIFO segment eviction. Selects the oldest segment and evicts it. As
    /// segments are append-only, this is similar to both slab LRU and slab LRC
    /// eviction strategies.
    Fifo,
    /// Closest to expiration. Selects the segment that would expire first and
    /// evicts it. This is a unique eviction strategy in segcache and
    /// effectively causes early expiration to free a segment.
    Cte,
    /// Least utilized segment. As segments are append-only, when an item is
    /// replaced or removed the segment containing that item now has dead bytes.
    /// This eviction strategy will free the segment that has the lowest number
    /// of live bytes. This strategy should cause the smallest impact to the
    /// number of live bytes held in the cache.
    Util,
    /// Merge eviction is a unique feature in segcache. It tries to retain items
    /// which have the biggest positive effect on hitrate.
    /// At its core, the idea is to take sequential segments in a chain,
    /// and merge their items into one segment. Unlike the NSDI paper, this
    /// implementation performs two different types of merge operations. The one
    /// matching the NSDI paper is used during eviction and may cause items to
    /// be evicted based on an estimate of their hit frequency. The other
    /// possible merge operation is a simple compaction which will combine
    /// segments which have low utilization (due to item replacement/deletion)
    /// without evicting any live items. Compaction has proven to be beneficial
    /// in workloads that frequently overwrite or delete items in the cache.
    Merge {
        /// The maximum number of segments to merge in a single pass. This can
        /// be used to bound the tail latency impact of a merge operation.
        max: usize,
        /// The target number of segments to merge during eviction. Setting this
        /// higher will result in fewer eviction passes and allow the algorithm
        /// to see more item frequencies. Setting this lower will cause fewer
        /// item evictions per pass.
        merge: usize,
        /// The target number of segments to merge during compaction. Compaction
        /// will only occur if a segment falls below `1/N`th occupancy. Setting
        /// this higher will cause fewer compaction runs but can result in a
        /// larger percentage of dead bytes.
        compact: usize,
    },
}

/// The `Eviction` struct is used to rank and return segments for eviction. It
/// implements eviction strategies corresponding to the `Policy`.
pub struct Eviction {
    policy: Policy,
    last_update_time: Instant,
    ranked_segs: Box<[Option<NonZeroU32>]>,
    index: usize,
    rng: Box<Random>,
}

impl Eviction {
    /// Creates a new `Eviction` struct which will handle up to `nseg` segments
    /// using the specified eviction policy.
    pub fn new(nseg: usize, policy: Policy) -> Self {
        let mut ranked_segs = Vec::with_capacity(0);
        ranked_segs.reserve_exact(nseg);
        ranked_segs.resize_with(nseg, || None);
        let ranked_segs = ranked_segs.into_boxed_slice();

        Self {
            policy,
            last_update_time: Instant::recent(),
            ranked_segs,
            index: 0,
            rng: Box::new(rng()),
        }
    }

    #[inline]
    pub fn policy(&self) -> Policy {
        self.policy
    }

    /// Returns the segment id of the least valuable segment
    pub fn least_valuable_seg(&mut self) -> Option<NonZeroU32> {
        let index = self.index;
        self.index += 1;
        if index < self.ranked_segs.len() {
            self.ranked_segs[index]
        } else {
            None
        }
    }

    /// Returns a random u32
    #[inline]
    pub fn random(&mut self) -> u32 {
        self.rng.gen()
    }

    pub fn should_rerank(&mut self) -> bool {
        let now = Instant::recent();
        match self.policy {
            Policy::None | Policy::Random | Policy::Merge { .. } => false,
            Policy::Fifo | Policy::Cte | Policy::Util => {
                if self.ranked_segs[0].is_none()
                    || (now - self.last_update_time).as_secs() > 1
                    || self.ranked_segs.len() < (self.index + 8)
                {
                    self.last_update_time = now;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub fn rerank(&mut self, headers: &[SegmentHeader]) {
        let mut ids: Vec<NonZeroU32> = headers.iter().map(|h| h.id()).collect();
        match self.policy {
            Policy::None | Policy::Random | Policy::Merge { .. } => {
                return;
            }
            Policy::Fifo { .. } => {
                ids.sort_by(|a, b| {
                    Self::compare_fifo(
                        &headers[a.get() as usize - 1],
                        &headers[b.get() as usize - 1],
                    )
                });
            }
            Policy::Cte { .. } => {
                ids.sort_by(|a, b| {
                    Self::compare_cte(
                        &headers[a.get() as usize - 1],
                        &headers[b.get() as usize - 1],
                    )
                });
            }
            Policy::Util { .. } => {
                ids.sort_by(|a, b| {
                    Self::compare_util(
                        &headers[a.get() as usize - 1],
                        &headers[b.get() as usize - 1],
                    )
                });
            }
        }
        for (i, id) in self.ranked_segs.iter_mut().enumerate() {
            *id = Some(ids[i]);
        }
        self.index = 0;
    }

    fn compare_fifo(lhs: &SegmentHeader, rhs: &SegmentHeader) -> Ordering {
        if !lhs.can_evict() {
            Ordering::Greater
        } else if !rhs.can_evict() {
            Ordering::Less
        } else if max(lhs.create_at(), lhs.merge_at()) > max(rhs.create_at(), rhs.merge_at()) {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }

    fn compare_cte(lhs: &SegmentHeader, rhs: &SegmentHeader) -> Ordering {
        if !lhs.can_evict() {
            Ordering::Greater
        } else if !rhs.can_evict() {
            Ordering::Less
        } else if (lhs.create_at() + lhs.ttl()) > (rhs.create_at() + rhs.ttl()) {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }

    fn compare_util(lhs: &SegmentHeader, rhs: &SegmentHeader) -> Ordering {
        if !lhs.can_evict() {
            Ordering::Greater
        } else if !rhs.can_evict() {
            Ordering::Less
        } else if lhs.live_bytes() > rhs.live_bytes() {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }

    #[inline]
    /// Returns the maximum number of segments which can be merged during a
    /// single merge operation. Applies to both eviction and compaction merge
    /// passes.
    pub fn max_merge(&self) -> usize {
        if let Policy::Merge { max, .. } = self.policy {
            max
        } else {
            8
        }
    }

    #[inline]
    /// Returns the number of segments which should be combined during an
    /// eviction merge.
    pub fn n_merge(&self) -> usize {
        if let Policy::Merge { merge, .. } = self.policy {
            merge
        } else {
            4
        }
    }

    #[inline]
    /// Returns the number of segments which should be combined during a
    /// compaction merge.
    pub fn n_compact(&self) -> usize {
        if let Policy::Merge { compact, .. } = self.policy {
            compact
        } else {
            2
        }
    }

    #[inline]
    /// The compact ratio serves as a low watermark for triggering compaction
    /// and combining segments without eviction.
    pub fn compact_ratio(&self) -> f64 {
        1.0 / self.n_compact() as f64
    }

    #[inline]
    /// The target ratio is used during eviction based merging and represents
    /// the desired occupancy of a segment once least accessed items are
    /// evicted.
    pub fn target_ratio(&self) -> f64 {
        1.0 / self.n_merge() as f64
    }

    #[inline]
    /// The stop ratio is used during merging as a high watermark and causes
    /// the merge pass to stop when the target segment has a higher occupancy
    pub fn stop_ratio(&self) -> f64 {
        self.target_ratio() * (self.n_merge() - 1) as f64 + 0.05
    }
}
