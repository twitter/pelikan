// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

use rustcommon_time::CoarseInstant as Instant;

const N_ITEM_SLOT: usize = 8;

// Maximum number of buckets in a chain, must be <= 255. Stored as a u64 to
// avoid repeated resizing for comparison
const MAX_CHAIN_LEN: u64 = 16;

#[derive(Copy, Clone)]
pub struct HashBucket {
    data: [u64; N_ITEM_SLOT],
}

impl HashBucket {
    fn new() -> Self {
        Self {
            data: [0; N_ITEM_SLOT],
        }
    }
}

#[repr(C)]
pub struct HashTable<S: BuildHasher> {
    hash_builder: Box<S>, // boxed so the size is consistent independent of type
    power: u64,
    mask: u64,
    data: Box<[HashBucket]>,
    rng: Box<Random>,
    started: CoarseInstant,
    next_to_chain: u64,
}

impl<S> HashTable<S>
where
    S: BuildHasher,
{
    pub fn with_hasher(power: u8, overflow_factor: f64, hash_builder: S) -> HashTable<S> {
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
        Self {
            hash_builder: Box::new(hash_builder),
            power: power.into(),
            mask,
            data: data.into_boxed_slice(),
            rng: Box::new(rng()),
            started: Instant::recent(),
            next_to_chain: buckets as u64,
        }
    }

    // get the item info for a key if it exists in the hash
    pub fn get(&mut self, key: &[u8], segments: &mut Segments) -> Option<Item> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        trace!("hash: {} mask: {} bucket: {}", hash, self.mask, bucket_id);

        let curr_ts = (Instant::recent() - self.started).as_secs() as u64 & PROC_TS_MASK;
        if curr_ts != get_ts(bucket.data[0]) {
            bucket.data[0] = (bucket.data[0] & !TS_MASK) | (curr_ts << TS_BIT_SHIFT);

            loop {
                let n_item_slot = if chain_idx == chain_len {
                    N_ITEM_SLOT
                } else {
                    N_ITEM_SLOT - 1
                };

                for i in 0..n_item_slot {
                    if chain_idx == 0 && i == 0 {
                        continue;
                    }
                    bucket.data[i] &= CLEAR_FREQ_SMOOTH_MASK;
                }

                if chain_idx == chain_len {
                    break;
                }
                bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
                chain_idx += 1;
            }

            // reset to start of chain
            chain_idx = 0;
            bucket = &mut self.data[bucket_id as usize];
        }

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };

            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }

                let current_info = bucket.data[i];

                if get_tag(current_info) == tag {
                    let current_item = segments.get_item(current_info).unwrap();
                    if current_item.key() != key {
                        increment_counter!(&Stat::HashTagCollision);
                    } else {
                        // update item frequency
                        let mut freq = get_freq(current_info);
                        if freq < 127 {
                            let rand: u64 = self.rng.gen();
                            if freq <= 16 || rand % freq == 0 {
                                freq = ((freq + 1) | 0x80) << FREQ_BIT_SHIFT;
                            } else {
                                freq = (freq | 0x80) << FREQ_BIT_SHIFT;
                            }
                            if bucket.data[i] == current_info {
                                bucket.data[i] = (current_info & !FREQ_MASK) | freq;
                            }
                        }

                        let item = Item::new(
                            current_item,
                            get_cas(self.data[(hash & self.mask) as usize].data[0]),
                        );
                        item.check_magic();

                        return Some(item);
                    }
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        None
    }

    // get the item info for a key if it exists in the hash without incrementing
    // the item frequency
    pub fn get_no_freq_incr(&mut self, key: &[u8], segments: &mut Segments) -> Option<Item> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        trace!("hash: {} mask: {} bucket: {}", hash, self.mask, bucket_id);

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };

            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }

                let current_info = bucket.data[i];

                if get_tag(current_info) == tag {
                    let current_item = segments.get_item(current_info).unwrap();
                    if current_item.key() != key {
                        increment_counter!(&Stat::HashTagCollision);
                    } else {
                        let item = Item::new(
                            current_item,
                            get_cas(self.data[(hash & self.mask) as usize].data[0]),
                        );
                        item.check_magic();

                        return Some(item);
                    }
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        None
    }

    // TODO(bmartin): decide on what width to actually return here...
    pub fn get_freq(&mut self, key: &[u8], segment: &mut Segment, offset: u64) -> Option<u64> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };

            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_info = bucket.data[i];

                // we can't actually check for a collision here since the item
                // may be in another segment, so just check if it's at the same
                // offset in the segment and treat that as a match
                if get_tag(current_info) == tag
                    && get_seg_id(current_info) == segment.id()
                    && get_offset(current_info) == offset
                {
                    return Some(get_freq(current_info) & 0x7F);
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        None
    }

    #[allow(clippy::result_unit_err)]
    pub fn relink_item(
        &mut self,
        key: &[u8],
        old_seg: i32,
        new_seg: i32,
        old_offset: u64,
        new_offset: u64,
    ) -> Result<(), ()> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        let mut updated = false;

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };

            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_info = bucket.data[i];

                if get_tag(current_info) == tag {
                    if get_seg_id(current_info) == old_seg && get_offset(current_info) == old_offset
                    {
                        if !updated {
                            let new_item_info = build_item_info(tag, new_seg as u64, new_offset);
                            bucket.data[i] = new_item_info;
                            updated = true;
                        } else {
                            bucket.data[i] = 0;
                        }
                    } else {
                        increment_counter!(&Stat::HashTagCollision);
                    }
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        if updated {
            increment_counter!(&Stat::ItemRelink);
            Ok(())
        } else {
            Err(())
        }
    }

    #[allow(clippy::result_unit_err)]
    pub fn insert(
        &mut self,
        item: RawItem,
        seg: i32,
        offset: u64,
        ttl_buckets: &mut TtlBuckets,
        segments: &mut Segments,
    ) -> Result<(), ()> {
        increment_counter!(&Stat::HashInsert);

        let hash = self.hash(&item.key());
        let tag = tag_from_hash(hash);
        let mut bucket_id = (hash & self.mask) as usize;
        let chain_len = chain_len(self.data[bucket_id].data[0]);
        let mut chain_idx = 0;

        // check the item magic
        item.check_magic();

        let mut insert_item_info = build_item_info(tag, seg as u64, offset);

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };

            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_item_info = self.data[bucket_id].data[i];
                if get_tag(current_item_info) != tag {
                    if insert_item_info != 0 && current_item_info == 0 {
                        // found a blank slot
                        self.data[bucket_id].data[i] = insert_item_info;
                        insert_item_info = 0;
                    }
                    continue;
                }
                if segments.get_item(current_item_info).unwrap().key() != item.key() {
                    increment_counter!(&Stat::HashTagCollision);
                } else {
                    // update existing key
                    self.data[bucket_id].data[i] = insert_item_info;
                    increment_counter!(&Stat::ItemReplace);
                    let _ = segments.remove_item(current_item_info, true, ttl_buckets, self);
                    insert_item_info = 0;
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket_id = self.data[bucket_id].data[N_ITEM_SLOT - 1] as usize;
            chain_idx += 1;
        }

        if insert_item_info != 0
            && chain_len < MAX_CHAIN_LEN
            && (self.next_to_chain as usize) < self.data.len()
        {
            let next_id = self.next_to_chain as usize;
            self.next_to_chain += 1;

            self.data[next_id].data[0] = self.data[bucket_id].data[N_ITEM_SLOT - 1];
            self.data[next_id].data[1] = insert_item_info;
            insert_item_info = 0;
            self.data[bucket_id].data[N_ITEM_SLOT - 1] = next_id as u64;

            self.data[(hash & self.mask) as usize].data[0] += 0x0001_0000_0000_0000;
        }

        if insert_item_info == 0 {
            self.data[(hash & self.mask) as usize].data[0] += 1;
            Ok(())
        } else {
            increment_counter!(&Stat::HashInsertEx);
            Err(())
        }
    }

    pub fn try_update_cas(
        &mut self,
        key: &[u8],
        cas: u32,
        segments: &mut Segments,
    ) -> Result<(), SegCacheError> {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        trace!("hash: {} mask: {} bucket: {}", hash, self.mask, bucket_id);

        if cas != get_cas(bucket.data[0]) {
            return Err(SegCacheError::Exists);
        }

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };
            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_info = bucket.data[i];

                if get_tag(current_info) == tag {
                    let current_item = segments.get_item(current_info).unwrap();
                    if current_item.key() != key {
                        increment_counter!(&Stat::HashTagCollision);
                    } else {
                        // update item frequency
                        let mut freq = get_freq(current_info);
                        if freq < 127 {
                            let rand: u64 = self.rng.gen();
                            if freq <= 16 || rand % freq == 0 {
                                freq = ((freq + 1) | 0x80) << FREQ_BIT_SHIFT;
                            } else {
                                freq = (freq | 0x80) << FREQ_BIT_SHIFT;
                            }
                            if bucket.data[i] == current_info {
                                bucket.data[i] = (current_info & !FREQ_MASK) | freq;
                            }
                        }

                        if cas == get_cas(bucket.data[0]) {
                            // TODO(bmartin): what is expected on overflow of the cas bits?
                            self.data[(hash & self.mask) as usize].data[0] += 1;
                            return Ok(());
                        }
                    }
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        Err(SegCacheError::NotFound)
    }

    pub fn delete(
        &mut self,
        key: &[u8],
        ttl_buckets: &mut TtlBuckets,
        segments: &mut Segments,
    ) -> bool {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let mut bucket_id = (hash & self.mask) as usize;
        let chain_len = chain_len(self.data[bucket_id].data[0]);
        let mut chain_idx = 0;

        let mut deleted = false;

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };
            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_item_info = self.data[bucket_id].data[i];

                if get_tag(current_item_info) == tag {
                    let current_item = segments.get_item(current_item_info).unwrap();
                    if current_item.key() != key {
                        increment_counter!(&Stat::HashTagCollision);
                        continue;
                    } else {
                        increment_counter!(&Stat::HashRemove);
                        let _ =
                            segments.remove_item(current_item_info, !deleted, ttl_buckets, self);
                        self.data[bucket_id].data[i] = 0;
                        deleted = true;
                    }
                }
            }

            if chain_idx == chain_len {
                break;
            }
            bucket_id = self.data[bucket_id].data[N_ITEM_SLOT - 1] as usize;
            chain_idx += 1;
        }

        if deleted {
            increment_counter!(&Stat::ItemDelete);
        }

        deleted
    }

    pub fn evict(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let result = self.remove_from(key, offset, segment);
        if result {
            increment_counter!(&Stat::ItemEvict);
        }
        result
    }

    pub fn expire(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let result = self.remove_from(key, offset, segment);
        if result {
            increment_counter!(&Stat::ItemExpire);
        }
        result
    }

    fn remove_from(&mut self, key: &[u8], offset: i32, segment: &mut Segment) -> bool {
        let hash = self.hash(key);
        let tag = tag_from_hash(hash);
        let bucket_id = hash & self.mask;

        let mut bucket = &mut self.data[bucket_id as usize];
        let chain_len = chain_len(bucket.data[0]);
        let mut chain_idx = 0;

        let evict_item_info = build_item_info(tag, segment.id() as u64, offset as u64);

        let mut evicted = false;
        let mut outdated = true;
        let mut first_match = true;

        loop {
            let n_item_slot = if chain_idx == chain_len {
                N_ITEM_SLOT
            } else {
                N_ITEM_SLOT - 1
            };
            for i in 0..n_item_slot {
                if chain_idx == 0 && i == 0 {
                    continue;
                }
                let current_item_info = clear_freq(bucket.data[i]);
                if get_tag(current_item_info) != tag {
                    continue;
                }

                if get_seg_id(current_item_info) == segment.id() {
                    let current_item = segment.get_item(current_item_info).unwrap();
                    if current_item.key() != key {
                        increment_counter!(&Stat::HashTagCollision);
                        continue;
                    }

                    if first_match {
                        if evict_item_info == current_item_info {
                            segment.remove_item(current_item_info, false);
                            bucket.data[i] = 0;
                            outdated = false;
                            evicted = true;
                        }
                        first_match = false;
                        continue;
                    } else {
                        if !evicted && current_item_info == evict_item_info {
                            evicted = true;
                        }
                        segment.remove_item(bucket.data[i], !outdated);
                        bucket.data[i] = 0;
                    }
                }
            }
            if chain_idx == chain_len {
                break;
            }
            bucket = &mut self.data[bucket.data[N_ITEM_SLOT - 1] as usize];
            chain_idx += 1;
        }

        evicted
    }

    fn hash(&self, key: &[u8]) -> u64 {
        increment_counter!(&Stat::HashLookup);
        let mut hasher = self.hash_builder.build_hasher();
        hasher.write(key);
        hasher.finish()
    }
}

#[inline]
pub const fn tag_from_hash(hash: u64) -> u64 {
    (hash & TAG_MASK) | 0x0010000000000000
}

#[inline]
pub const fn get_offset(item_info: u64) -> u64 {
    (item_info & OFFSET_MASK) << OFFSET_UNIT_IN_BIT
}

#[inline]
pub const fn get_seg_id(item_info: u64) -> i32 {
    ((item_info & SEG_ID_MASK) >> SEG_ID_BIT_SHIFT) as i32
}

#[inline]
pub const fn get_freq(item_info: u64) -> u64 {
    (item_info & FREQ_MASK) >> FREQ_BIT_SHIFT
}

#[inline]
pub const fn get_cas(bucket_info: u64) -> u32 {
    (bucket_info & CAS_MASK) as u32
}

#[inline]
pub const fn get_ts(bucket_info: u64) -> u64 {
    (bucket_info & TS_MASK) >> TS_BIT_SHIFT
}

#[inline]
pub const fn get_tag(item_info: u64) -> u64 {
    item_info & TAG_MASK
}

#[inline]
pub const fn clear_freq(item_info: u64) -> u64 {
    item_info & !FREQ_MASK
}

#[inline]
pub const fn chain_len(bucket_info: u64) -> u64 {
    (bucket_info & BUCKET_CHAIN_LEN_MASK) >> BUCKET_CHAIN_LEN_BIT_SHIFT
}

#[inline]
pub const fn build_item_info(tag: u64, seg_id: u64, offset: u64) -> u64 {
    tag | (seg_id << SEG_ID_BIT_SHIFT) | (offset >> OFFSET_UNIT_IN_BIT)
}
