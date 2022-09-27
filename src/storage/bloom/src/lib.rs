//! This library contains an implementation of a bloom filter.
//! 
//! There are two types exported by this library:
//! - [`BloomFilter`] is a typed bloom filter that allows for inserting keys
//!   and probabilistically checking whether they are present. This is what
//!   you should use by default.
//! - [`RawBloomFilter`] implements the core of the bloom filter datastructure
//!   but it requires you to provide the two hashes used for each element. Use
//!   this if you need absolute control over how items are placed in the bloom
//!   filter or if you need to store items with different types into the same
//!   bloom filter.
//! 
//! # Choosing bloom filter parameters
//! A bloom filter only has two parameters:
//! - _k_ - the number of bits that we set in the bloom filter for each value
//!   inserted, and
//! - _m_ - the size of the bloom filter in bits.
//! 
//! To figure out what these should be you need your desired error rate 
//! _ε_ and a rough estimate of _n_, the number of elements that will be
//! stored in the bloom filter. Then, the optimal values of _k_ and _m_ are
//! given by
//! - _k_ = (_m_ / _n_) ln(2)
//! - _m_ = (_n_ ln _ε_) / (ln 2)<sup>2</sup>
//! 

use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bitvec::prelude::BitVec;
use metrohash::MetroHash64;
use twox_hash::Xxh3Hash64;

fn xxh3hash<T: Hash + ?Sized>(value: &T, seed: u64) -> u64 {
    let mut hasher = Xxh3Hash64::with_seed(seed);
    value.hash(&mut hasher);
    hasher.finish()
}

fn metrohash<T: Hash + ?Sized>(value: &T, seed: u64) -> u64 {
    let mut hasher = MetroHash64::with_seed(seed);
    value.hash(&mut hasher);
    hasher.finish()
}

/// Low-level bloom filter implementation that directly uses element hashes.
#[derive(Clone)]
pub struct RawBloomFilter {
    bits: BitVec,
    k: u64,
}

impl RawBloomFilter {
    /// Create a new bloom filter with `m` bits that stores `k` hashes for each
    /// value inserted.
    /// 
    /// # Panics
    /// Panics if
    /// - `m` is 0
    /// - `k` is 0
    /// - `k` > `m`
    /// - `m` is not a multiple of `usize::BITS`
    pub fn new(m: usize, k: usize) -> Self {
        assert_ne!(m, 0, "m must be greater than 0");
        assert_ne!(k, 0, "k must be greater than 0");
        assert!(k <= m, "m must be greater than k (got {k} > {m})");
        assert!(
            m % usize::BITS as usize == 0,
            "len must be a multiple of usize::BITS"
        );

        Self {
            bits: BitVec::repeat(false, m),
            k: k as u64,
        }
    }

    /// Insert the value corresponding to the two provided hashes.
    pub fn insert(&mut self, hash1: u64, hash2: u64) {
        for index in self.indices(hash1, hash2) {
            self.bits.set(index, true);
        }
    }

    /// Check whether this bloom filter contains the value corresponding
    /// to these two hashes.
    pub fn contains(&self, hash1: u64, hash2: u64) -> bool {
        self.indices(hash1, hash2).all(|index| self.bits[index])
    }

    /// Erase all items from the bloom filter.
    pub fn clear(&mut self) {
        self.bits.fill(false);
    }

    pub fn from_parts(data: BitVec, k: usize) -> Self {
        Self {
            bits: data,
            k: k as u64,
        }
    }

    /// Compute the bit indices within the bloom filter for the provided values.
    fn indices(&self, hash1: u64, hash2: u64) -> impl Iterator<Item = usize> {
        // Instead of coming up with k different hash functinos we can use linear
        // combinations to create an unlimited number of hashes from only two initial
        // hashes.
        //
        // To do this we first calculate two hashes, h1 and h2, then we form the rest of
        // the hashes like this:
        //   g_i = h1 + i * h2
        //
        // This ends up with the same distribution properties as just evaluating k
        // different hash functions but it is far cheaper to execute.
        //
        // The source for this is "Less hashing, same performance: Building a better
        // bloom filter" by Kirtz and Mitzenmacher.
        let len = self.bits.len();
        (0..self.k)
            .map(move |i| hash1.wrapping_add(hash2.wrapping_mul(i)))
            .map(move |hash| (hash % len as u64) as usize)
    }
}

/// Bloom filter.
pub struct BloomFilter<T: ?Sized> {
    raw: RawBloomFilter,
    seed: u64,
    _dummy: PhantomData<*const T>,
}

impl<T: Hash + ?Sized> BloomFilter<T> {
    /// Create a new bloom filter with a random seed and `m` bits which stores
    /// `k` hashes for each inserted element.
    /// 
    /// Note that `m` will be rounded up to the next multiple of `usize::BITS`.
    /// 
    /// # Panics
    /// Panics if
    /// - `m` is 0
    /// - `k` is 0
    /// - `k` is greater than `m`
    #[cfg(feature = "rand")]
    pub fn new(m: usize, k: usize) -> Self {
        Self::with_seed(m, k, rand::random())
    }

    /// Create a bloom filter with the provided seen and `m` bits which stores
    /// `k` hashes for each inserted element.
    /// 
    /// Note that `m` will be rounded up to the next multiple of `usize::BITS`.
    /// 
    /// # Panics
    /// Panics if
    /// - `m` is 0
    /// - `k` is 0
    /// - `k` is greater than `m`
    pub fn with_seed(m: usize, k: usize, seed: u64) -> Self {
        const MASK: usize = (usize::BITS - 1) as usize;

        Self {
            // Note that we round the size of the bloom filter up to the next
            // word size for convenience.
            raw: RawBloomFilter::new((m + MASK) & !MASK, k),
            seed,
            _dummy: PhantomData,
        }
    }

    /// Hash a value into the two hashes used by the bloom filter.
    fn hash_value(&self, value: &T) -> [u64; 2] {
        [xxh3hash(value, self.seed), metrohash(value, self.seed)]
    }

    /// Insert a value into this bloom filter.
    pub fn insert(&mut self, value: &T) {
        let [hash1, hash2] = self.hash_value(value);
        self.raw.insert(hash1, hash2)
    }

    /// Check whether this bloom filter contains a given value.
    /// 
    /// This may return true even if `value` has not been inserted into the
    /// filter.
    pub fn contains(&self, value: &T) -> bool {
        let [hash1, hash2] = self.hash_value(value);
        self.raw.contains(hash1, hash2)
    }

    /// Erase all items from the bloom filter.
    pub fn clear(&mut self) {
        self.raw.clear()
    }
}

impl<T: ?Sized> Clone for BloomFilter<T> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            seed: self.seed,
            _dummy: PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let mut bloom = RawBloomFilter::new(64, 4);

        bloom.insert(0, 3);

        assert!(bloom.contains(0, 3));
    }

    #[test]
    fn empty_by_default() {
        let bloom = RawBloomFilter::new(64, 4);

        assert!(!bloom.contains(0, 4));
        assert!(!bloom.contains(5, 7));
    }

    #[test]
    fn missing() {
        let mut bloom = RawBloomFilter::new(64, 4);

        bloom.insert(5, 3);

        assert!(!bloom.contains(0, 4));
        assert!(!bloom.contains(5, 7));
    }

    #[test]
    fn collision() {
        let mut bloom = RawBloomFilter::new(64, 8);

        bloom.insert(0, 8);
        bloom.insert(4, 8);

        assert!(bloom.contains(24, 4));
    }

    #[test]
    fn clear() {
        let mut bloom = RawBloomFilter::new(64, 8);

        bloom.insert(0, 8);
        bloom.insert(7, 3);

        assert!(bloom.contains(0, 8));
        assert!(bloom.contains(7, 3));

        bloom.clear();

        assert!(!bloom.contains(0, 8));
        assert!(!bloom.contains(7, 3));
    }
}
