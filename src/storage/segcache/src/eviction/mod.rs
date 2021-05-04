// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Eviction is used to select a segment to remove when the cache becomes full.
//! An eviction [`Policy`] determines what data will be evicted from the cache.

use crate::rng;
use crate::segments::*;
use crate::Random;
use core::cmp::max;
use core::cmp::Ordering;
use core::num::NonZeroU32;
use rand::Rng;

use rustcommon_time::CoarseInstant as Instant;

mod policy;

pub use policy::Policy;

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
