use crate::segments::*;
use core::cmp::max;
use core::cmp::Ordering;

use rustcommon_time::CoarseInstant as Instant;

/// `Policy` defines the eviction strategy to be used.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Policy {
    /// No eviction
    None,
    /// Segment random eviction
    Random,
    /// FIFO segment eviction
    Fifo,
    /// Closest to expiration
    Cte,
    /// Least utilized segment
    Util,
    /// Merge eviction
    Merge {
        max: usize,
        merge: usize,
        compact: usize,
    },
}

/// The `Eviction` struct is used to rank and return segments for eviction. It
/// implements eviction strategies corresponding to the `Policy`.
pub struct Eviction {
    policy: Policy,
    last_update_time: Instant,
    ranked_segs: Box<[i32]>,
    index: usize,
    rng: Box<Random>,
}

impl Eviction {
    /// Creates a new `Eviction` struct which will handle up to `nseg` segments
    /// using the specified eviction policy.
    pub fn new(nseg: usize, policy: Policy) -> Self {
        let mut ranked_segs = Vec::with_capacity(0);
        ranked_segs.reserve_exact(nseg);
        ranked_segs.resize_with(nseg, || -1);
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
    pub fn least_valuable_seg(&mut self) -> Option<i32> {
        let index = self.index;
        self.index += 1;
        if index < self.ranked_segs.len() {
            self.ranked_segs[index].as_option()
        } else {
            None
        }
    }

    /// Returns a random i32
    #[inline]
    pub fn random(&mut self) -> i32 {
        self.rng.gen()
    }

    pub fn should_rerank(&mut self) -> bool {
        let now = Instant::recent();
        match self.policy {
            Policy::None | Policy::Random | Policy::Merge { .. } => false,
            Policy::Fifo | Policy::Cte | Policy::Util => {
                if self.ranked_segs[0].as_option().is_none()
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
        let mut ids: Vec<i32> = headers.iter().map(|h| h.id()).collect();
        match self.policy {
            Policy::None | Policy::Random | Policy::Merge { .. } => {
                return;
            }
            Policy::Fifo { .. } => {
                ids.sort_by(|a, b| {
                    Self::compare_fifo(&headers[*a as usize], &headers[*b as usize])
                });
            }
            Policy::Cte { .. } => {
                ids.sort_by(|a, b| Self::compare_cte(&headers[*a as usize], &headers[*b as usize]));
            }
            Policy::Util { .. } => {
                ids.sort_by(|a, b| {
                    Self::compare_util(&headers[*a as usize], &headers[*b as usize])
                });
            }
        }
        for (i, id) in self.ranked_segs.iter_mut().enumerate() {
            *id = ids[i];
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
        } else if lhs.occupied_size() > rhs.occupied_size() {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }

    #[inline]
    pub fn max_merge(&self) -> usize {
        if let Policy::Merge { max, .. } = self.policy {
            max
        } else {
            8
        }
    }

    #[inline]
    pub fn n_merge(&self) -> usize {
        if let Policy::Merge { merge, .. } = self.policy {
            merge
        } else {
            4
        }
    }

    #[inline]
    pub fn n_compact(&self) -> usize {
        if let Policy::Merge { compact, .. } = self.policy {
            compact
        } else {
            2
        }
    }

    #[inline]
    pub fn compact_ratio(&self) -> f64 {
        1.0 / self.n_compact() as f64
    }

    #[inline]
    pub fn target_ratio(&self) -> f64 {
        1.0 / self.n_merge() as f64
    }

    #[inline]
    pub fn stop_ratio(&self) -> f64 {
        self.target_ratio() * (self.n_merge() - 1) as f64 + 0.05
    }
}
