// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A hashtable is used to lookup items and store per-item metadata.
//!
//! The [`HashTable`] design uses bulk chaining to reduce the per item overheads,
//! share metadata where possible, and provide better data locality.
//!
//! For a more detailed description of the implementation, please see:
//! <https://twitter.github.io/pelikan/2021/segcache.html>
//!
//! Our [`HashTable`] is composed of a base unit called a [`HashBucket`]. Each
//! bucket is a contiguous allocation that is sized to fit in a single
//! cacheline. This gives us room for a total of 8 64bit slots within the
//! bucket. The first slot of a bucket is used for per bucket metadata, leaving
//! us with up to 7 slots for items in the bucket:
//!
//! ```text
//!    ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
//!    │Bucket│ Item │ Item │ Item │ Item │ Item │ Item │ Item │
//!    │ Info │ Info │ Info │ Info │ Info │ Info │ Info │ Info │
//!    │      │      │      │      │      │      │      │      │
//!    │64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│
//!    │      │      │      │      │      │      │      │      │
//!    └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘
//! ```
//!
//! When a bucket is full, we may be able to chain another bucket from the
//! overflow area onto the primary bucket. To store a pointer to the next bucket
//! in the chain, we reduce the item capacity of the bucket and store the
//! pointer in the last slot. This can be repeated to chain additional buckets:
//!
//! ```text
//!    ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
//!    │Bucket│ Item │ Item │ Item │ Item │ Item │ Item │ Next │
//!    │ Info │ Info │ Info │ Info │ Info │ Info │ Info │Bucket│
//!    │      │      │      │      │      │      │      │      │──┐
//!    │64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│  │
//!    │      │      │      │      │      │      │      │      │  │
//!    └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘  │
//!                                                               │
//! ┌─────────────────────────────────────────────────────────────┘
//! │
//! │  ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
//! │  │ Item │ Item │ Item │ Item │ Item │ Item │ Item │ Next │
//! │  │ Info │ Info │ Info │ Info │ Info │ Info │ Info │Bucket│
//! └─▶│      │      │      │      │      │      │      │      │──┐
//!    │64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│  │
//!    │      │      │      │      │      │      │      │      │  │
//!    └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘  │
//!                                                               │
//! ┌─────────────────────────────────────────────────────────────┘
//! │
//! │  ┌──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────┐
//! │  │ Item │ Item │ Item │ Item │ Item │ Item │ Item │ Item │
//! │  │ Info │ Info │ Info │ Info │ Info │ Info │ Info │ Info │
//! └─▶│      │      │      │      │      │      │      │      │
//!    │64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│64 bit│
//!    │      │      │      │      │      │      │      │      │
//!    └──────┴──────┴──────┴──────┴──────┴──────┴──────┴──────┘
//! ```
//!
//! This works out so that we have capacity to store 7 items for every bucket
//! allocated to a chain.
//!

// hashtable

/// The number of slots within each bucket
const N_BUCKET_SLOT: usize = 8;

/// Maximum number of buckets in a chain. Must be <= 255.
const MAX_CHAIN_LEN: u64 = 16;

use crate::*;
use ahash::RandomState;
use core::marker::PhantomData;
use core::num::NonZeroU32;

mod hash_bucket;

pub(crate) use hash_bucket::*;

counter!(HASH_TAG_COLLISION, "number of partial hash collisions");
counter!(HASH_INSERT, "number of inserts into the hash table");
counter!(
    HASH_INSERT_EX,
    "number of hash table inserts which failed, likely due to capacity"
);
counter!(
    HASH_REMOVE,
    "number of hash table entries which have been removed"
);
counter!(
    HASH_LOOKUP,
    "total number of lookups against the hash table"
);
counter!(
    ITEM_RELINK,
    "number of times items have been relinked to different locations"
);
counter!(ITEM_REPLACE, "number of times items have been replaced");
counter!(ITEM_DELETE, "number of items removed from the hash table");
counter!(ITEM_EXPIRE, "number of items removed due to expiration");
counter!(ITEM_EVICT, "number of items removed due to eviction");

#[derive(Debug)]
struct IterState {
    bucket_id: usize,
    buckets_len: usize,
    item_slot: usize,
    chain_len: usize,
    chain_idx: usize,
    finished: bool,
}

impl IterState {
    fn new(hashtable: &HashTable, hash: u64) -> Self {
        let bucket_id = (hash & hashtable.mask) as usize;
        let buckets_len = hashtable.data.len();
        let bucket = hashtable.data[bucket_id];
        let chain_len = chain_len(bucket.data[0]) as usize;

        Self {
            bucket_id,
            buckets_len,
            // we start with item_slot 1 because slot 0 is metadata when in the
            // first bucket in the chain
            item_slot: 1,
            chain_len,
            chain_idx: 0,
            finished: false,
        }
    }

    fn n_item_slot(&self) -> usize {
        // if this is the last bucket in the chain, the final slot contains item
        // info entry, otherwise it points to the next bucket and should not be
        // treated as an item slot
        if self.chain_idx == self.chain_len {
            N_BUCKET_SLOT
        } else {
            N_BUCKET_SLOT - 1
        }
    }
}

struct IterMut<'a> {
    ptr: *mut HashBucket,
    state: IterState,
    // we need this marker to carry the lifetime since we must use a pointer
    // instead of a reference for the mutable variant of the iterator
    _marker: PhantomData<&'a mut u64>,
}

impl<'a> IterMut<'a> {
    fn new(hashtable: &'a mut HashTable, hash: u64) -> Self {
        let state = IterState::new(hashtable, hash);

        let ptr = hashtable.data.as_mut_ptr();

        Self {
            ptr,
            state,
            _marker: PhantomData,
        }
    }
}

impl<'a> Iterator for IterMut<'a> {
    type Item = &'a mut u64;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        if self.state.finished {
            return None;
        }

        let n_item_slot = self.state.n_item_slot();

        // SAFETY: this assert ensures memory safety for the pointer operations
        // that follow as in-line unsafe blocks. We first check to make sure the
        // bucket_id is within range for the slice of buckets. As long as this
        // holds true, the pointer operations are safe.
        assert!(
            self.state.bucket_id < self.state.buckets_len,
            "bucket id not in range"
        );

        let item_info =
            unsafe { &mut (*self.ptr.add(self.state.bucket_id)).data[self.state.item_slot] };

        if self.state.item_slot < n_item_slot - 1 {
            self.state.item_slot += 1;
        } else {
            // finished iterating in this bucket, see if it's chained
            if self.state.chain_idx < self.state.chain_len {
                self.state.chain_idx += 1;
                self.state.item_slot = 0;
                let next_bucket_id = unsafe {
                    (*self.ptr.add(self.state.bucket_id)).data[N_BUCKET_SLOT - 1] as usize
                };
                self.state.bucket_id = next_bucket_id;
            } else {
                self.state.finished = true;
            }
        }

        Some(item_info)
    }
}

/// Main structure for performing item lookup. Contains a contiguous allocation
/// of [`HashBucket`]s which are used to store item info and metadata.
#[repr(C)]
pub(crate) struct HashTable {
    hash_builder: Box<RandomState>,
    power: u64,
    mask: u64,
    data: Box<[HashBucket]>,
    started: Instant,
    next_to_chain: u64,
    _pad: [u8; 8],
}

impl HashTable {
    /// Creates a new hashtable with a specified power and overflow factor. The
    /// hashtable will have the capacity to store up to
    /// `7 * 2^(power - 3) * (1 + overflow_factor)` items.
    pub fn new(power: u8, overflow_factor: f64) -> HashTable {
        if overflow_factor < 0.0 {
            fatal!("hashtable overflow factor must be >= 0.0");
        }

        // overflow factor is effectively bounded by the max chain length
        if overflow_factor > MAX_CHAIN_LEN as f64 {
            fatal!("hashtable overflow factor must be <= {}", MAX_CHAIN_LEN);
        }

        let slots = 1_u64 << power;
        let buckets = slots / 8;
        let mask = buckets - 1;

        let total_buckets = (buckets as f64 * (1.0 + overflow_factor)).ceil() as usize;

        let mut data = Vec::with_capacity(0);
        data.reserve_exact(total_buckets as usize);
        data.resize(total_buckets as usize, HashBucket::new());
        debug!(
            "hashtable has: {} primary slots across {} primary buckets and {} total buckets",
            slots, buckets, total_buckets,
        );

        let hash_builder = RandomState::with_seeds(
            0xbb8c484891ec6c86,
            0x0522a25ae9c769f9,
            0xeed2797b9571bc75,
            0x4feb29c1fbbd59d0,
        );

        Self {
            hash_builder: Box::new(hash_builder),
            power: power.into(),
            mask,
            data: data.into_boxed_slice(),
            started: Instant::now(),
            next_to_chain: buckets as u64,
            _pad: [0; 8],
        }
    }

    /// Lookup an item by key and return it
    pub fn get(&mut self, key: &[u8], time: Instant, segments: &mut Segments) -> Option<Item> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let bucket_info = self.data[bucket_id as usize].data[0];

        let curr_ts = (time - self.started).as_secs() & PROC_TS_MASK;

        if curr_ts != get_ts(bucket_info) as u32 {
            self.data[bucket_id as usize].data[0] = (bucket_info & !TS_MASK) | (curr_ts as u64);

            let iter = IterMut::new(self, hash);
            for item_info in iter {
                *item_info &= CLEAR_FREQ_SMOOTH_MASK;
            }
        }

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                let current_item = segments.get_item(*item_info).unwrap();
                if current_item.key() != key {
                    HASH_TAG_COLLISION.increment();
                } else {
                    // update item frequency
                    let mut freq = get_freq(*item_info);
                    if freq < 127 {
                        let rand = thread_rng().gen::<u64>();
                        if freq <= 16 || rand % freq == 0 {
                            freq = ((freq + 1) | 0x80) << FREQ_BIT_SHIFT;
                        } else {
                            freq = (freq | 0x80) << FREQ_BIT_SHIFT;
                        }
                        *item_info = (*item_info & !FREQ_MASK) | freq;
                    }

                    let age = segments.get_age(*item_info).unwrap();
                    let item = Item::new(
                        current_item,
                        age,
                        get_cas(self.data[(hash & self.mask) as usize].data[0]),
                    );
                    item.check_magic();

                    return Some(item);
                }
            }
        }

        None
    }

    /// Lookup an item by key and return it
    pub fn get_age(&mut self, key: &[u8], segments: &mut Segments) -> Option<u32> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                let current_item = segments.get_item(*item_info).unwrap();
                if current_item.key() != key {
                    HASH_TAG_COLLISION.increment();
                } else {
                    return segments.get_age(*item_info);
                }
            }
        }

        None
    }

    /// Lookup an item by key and return it
    /// compare to get, this is designed to support multiple readers and single writer. 
    /// because eviction always remove hashtable entry first, 
    /// so if an object is evicted, its hash table entry must have been removed, 
    /// as a result, we can verify hash table entry after reading/copying the value.
    /// 
    /// Therefore, we can leverage opportunistic concurrency control to support
    /// multiple readers and a single writer. 
    /// we check the hash table after a reader reads the data, 
    /// if the data is evicted, then its hash table entry must have been removed.
    ///  
    pub fn get_with_item_info(&mut self, key: &[u8], time: Instant, segments: &mut Segments) -> Option<RichItem> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let bucket_info = self.data[bucket_id as usize].data[0];

        let curr_ts = (time - self.started).as_secs() & PROC_TS_MASK;

        if curr_ts != get_ts(bucket_info) as u32 {
            self.data[bucket_id as usize].data[0] = (bucket_info & !TS_MASK) | (curr_ts as u64);

            let iter = IterMut::new(self, hash);
            for item_info in iter {
                *item_info &= CLEAR_FREQ_SMOOTH_MASK;
            }
        }

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            let item_info_val = *item_info;
            if get_tag(item_info_val) == tag {
                let current_item = segments.get_item(*item_info).unwrap();
                if current_item.key() != key {
                    HASH_TAG_COLLISION.increment();
                } else {
                    // update item frequency
                    let mut freq = get_freq(*item_info);
                    if freq < 127 {
                        let rand = thread_rng().gen::<u64>();
                        if freq <= 16 || rand % freq == 0 {
                            freq = ((freq + 1) | 0x80) << FREQ_BIT_SHIFT;
                        } else {
                            freq = (freq | 0x80) << FREQ_BIT_SHIFT;
                        }
                        // TODO: this needs to be atomic
                        // worse case new item insert fails
                        *item_info = (*item_info & !FREQ_MASK) | freq;
                    }

                    let age = segments.get_age(item_info_val).unwrap();
                    let item = RichItem::new(
                        current_item,
                        age,
                        item_info_val & !FREQ_MASK,
                        item_info,
                        get_cas(self.data[(hash & self.mask) as usize].data[0]),
                    );
                    assert!(item.verify_hashtable_entry()); 
                    item.check_magic();

                    return Some(item);
                }
            }
        }

        None
    }    

    /// Lookup an item by key and return it without incrementing the item
    /// frequency. This may be used to compose higher-level functions which do
    /// not want a successful item lookup to count as a hit for that item.
    pub fn get_no_freq_incr(&mut self, key: &[u8], segments: &mut Segments) -> Option<Item> {
        let hash = self.hash(key);

        let iter = IterMut::new(self, hash);

        let tag = tag_from_hash(hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                let current_item = segments.get_item(*item_info).unwrap();
                if current_item.key() != key {
                    HASH_TAG_COLLISION.increment();
                } else {
                    let age = segments.get_age(*item_info).unwrap();
                    let item = Item::new(
                        current_item,
                        age,
                        get_cas(self.data[(hash & self.mask) as usize].data[0]),
                    );
                    item.check_magic();

                    return Some(item);
                }
            }
        }

        None
    }

    /// Return the frequency for the item with the key
    pub fn get_freq(&mut self, key: &[u8], segment: &mut Segment, offset: u64) -> Option<u64> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag
                && get_seg_id(*item_info) == Some(segment.id())
                && get_offset(*item_info) == offset
            {
                return Some(get_freq(*item_info) & 0x7F);
            }
        }

        None
    }

    /// Relinks the item to a new location
    #[allow(clippy::result_unit_err)]
    pub fn relink_item(
        &mut self,
        key: &[u8],
        old_seg: NonZeroU32,
        new_seg: NonZeroU32,
        old_offset: u64,
        new_offset: u64,
    ) -> Result<(), ()> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                if get_seg_id(*item_info) == Some(old_seg) && get_offset(*item_info) == old_offset {
                    *item_info = build_item_info(tag, new_seg, new_offset);
                    ITEM_RELINK.increment();
                    return Ok(());
                } else {
                    HASH_TAG_COLLISION.increment();
                }
            }
        }

        Err(())
    }

    pub(crate) fn is_item_at(&mut self, key: &[u8], seg: NonZeroU32, offset: u64) -> bool {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                if get_seg_id(*item_info) == Some(seg) && get_offset(*item_info) == offset {
                    return true;
                } else {
                    HASH_TAG_COLLISION.increment();
                }
            }
        }

        false
    }

    /// Inserts a new item into the hashtable. This may fail if the hashtable is
    /// full.
    #[allow(clippy::result_unit_err)]
    pub fn insert(
        &mut self,
        item: RawItem,
        seg: NonZeroU32,
        offset: u64,
        ttl_buckets: &mut TtlBuckets,
        segments: &mut Segments,
    ) -> Result<(), ()> {
        HASH_INSERT.increment();

        let hash = self.hash(item.key());
        let tag = tag_from_hash(hash);

        // check the item magic
        item.check_magic();

        let mut insert_item_info = build_item_info(tag, seg, offset);

        let mut removed: Option<u64> = None;

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) != tag {
                if insert_item_info != 0 && *item_info == 0 {
                    // found a blank slot
                    *item_info = insert_item_info;
                    insert_item_info = 0;
                }
                continue;
            }
            if segments.get_item(*item_info).unwrap().key() != item.key() {
                HASH_TAG_COLLISION.increment();
            } else {
                // update existing key
                removed = Some(*item_info);
                *item_info = insert_item_info;
                insert_item_info = 0;
                break;
            }
        }

        if let Some(removed_item) = removed {
            ITEM_REPLACE.increment();
            let _ = segments.remove_item(removed_item, ttl_buckets, self);
        }

        if insert_item_info != 0 {
            let mut bucket_id = (hash & self.mask) as usize;
            let chain_len = chain_len(self.data[bucket_id].data[0]);

            if chain_len < MAX_CHAIN_LEN && (self.next_to_chain as usize) < self.data.len() {
                // we need to chase through the buckets to get the id of the last
                // bucket in the chain
                for _ in 0..chain_len {
                    bucket_id = self.data[bucket_id].data[N_BUCKET_SLOT - 1] as usize;
                }

                let next_id = self.next_to_chain as usize;
                self.next_to_chain += 1;

                self.data[next_id].data[0] = self.data[bucket_id].data[N_BUCKET_SLOT - 1];
                self.data[next_id].data[1] = insert_item_info;
                insert_item_info = 0;
                self.data[bucket_id].data[N_BUCKET_SLOT - 1] = next_id as u64;

                self.data[(hash & self.mask) as usize].data[0] += 0x0000_0000_0001_0000;
            }
        }

        if insert_item_info == 0 {
            self.data[(hash & self.mask) as usize].data[0] += 1 << CAS_BIT_SHIFT;
            Ok(())
        } else {
            HASH_INSERT_EX.increment();
            Err(())
        }
    }

    /// Used to implement higher-level CAS operations. This function looks up an
    /// item by key and checks if the CAS value matches the provided value.
    ///
    /// A success indicates that the item was found with the CAS value provided
    /// and that the CAS value has now been updated to a new value.
    ///
    /// A failure indicates that the CAS value did not match or there was no
    /// matching item for that key.
    pub fn try_update_cas<'a>(
        &mut self,
        key: &'a [u8],
        cas: u32,
        segments: &mut Segments,
    ) -> Result<(), SegError> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            if get_tag(*item_info) == tag {
                let item = segments.get_item(*item_info).unwrap();
                if item.key() != key {
                    HASH_TAG_COLLISION.increment();
                } else {
                    // update item frequency
                    let mut freq = get_freq(*item_info);
                    if freq < 127 {
                        let rand = thread_rng().gen::<u64>();
                        if freq <= 16 || rand % freq == 0 {
                            freq = ((freq + 1) | 0x80) << FREQ_BIT_SHIFT;
                        } else {
                            freq = (freq | 0x80) << FREQ_BIT_SHIFT;
                        }
                        *item_info = (*item_info & !FREQ_MASK) | freq;
                    }

                    if cas == get_cas(self.data[bucket_id as usize].data[0]) {
                        self.data[bucket_id as usize].data[0] += 1 << CAS_BIT_SHIFT;
                        return Ok(());
                    } else {
                        return Err(SegError::Exists);
                    }
                }
            }
        }

        Err(SegError::NotFound)
    }

    /// Removes the item with the given key
    pub fn delete(
        &mut self,
        key: &[u8],
        ttl_buckets: &mut TtlBuckets,
        segments: &mut Segments,
    ) -> bool {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);

        let iter = IterMut::new(self, hash);

        let mut removed: Option<u64> = None;

        for item_info in iter {
            if get_tag(*item_info) == tag {
                let item = segments.get_item(*item_info).unwrap();
                if item.key() != key {
                    HASH_TAG_COLLISION.increment();
                    continue;
                } else {
                    HASH_REMOVE.increment();
                    removed = Some(*item_info);
                    *item_info = 0;
                    break;
                }
            }
        }

        if let Some(removed_item) = removed {
            ITEM_DELETE.increment();
            let _ = segments.remove_item(removed_item, ttl_buckets, self);
            true
        } else {
            false
        }
    }

    /// Evict a single item from the cache
    pub fn evict(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let result = self.remove_from(key, offset, segment);
        if result {
            ITEM_EVICT.increment();
        }
        result
    }

    /// Expire a single item from the cache
    pub fn expire(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let result = self.remove_from(key, offset, segment);
        if result {
            ITEM_EXPIRE.increment();
        }
        result
    }

    /// Internal function that removes an item from a segment
    fn remove_from(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let evict_item_info = build_item_info(tag, segment.id(), offset as u64);

        let iter = IterMut::new(self, hash);

        for item_info in iter {
            let current_item_info = clear_freq(*item_info);
            if get_tag(current_item_info) != tag {
                continue;
            }

            if get_seg_id(current_item_info) != Some(segment.id())
                || get_offset(current_item_info) != offset as u64
            {
                HASH_TAG_COLLISION.increment();
                continue;
            }

            if evict_item_info == current_item_info {
                segment.remove_item(current_item_info);
                *item_info = 0;
                return true;
            }
        }

        false
    }

    /// Internal function used to calculate a hash value for a key
    fn hash(&self, key: &[u8]) -> u64 {
        HASH_LOOKUP.increment();
        let mut hasher = self.hash_builder.build_hasher();
        hasher.write(key);
        hasher.finish()
    }
}
