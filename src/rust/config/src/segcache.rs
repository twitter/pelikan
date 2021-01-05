// Copyright 2020 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

// constants to define default values
const HASH_POWER: u8 = 16;
const HASH_EXTRA_CAPACITY: f64 = 0.0;
const SEG_SIZE: i32 = 1024 * 1024;
const SEGMENTS: i32 = 64;
const EVICTION: Eviction = Eviction::Merge;
const COMPACT_TARGET: usize = 2;
const MERGE_TARGET: usize = 4;
const MERGE_MAX: usize = 8;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum Eviction {
    None,
    Random,
    Fifo,
    Cte,
    Util,
    Merge,
}

// helper functions for default values
fn hash_power() -> u8 {
    HASH_POWER
}

fn hash_extra_capacity() -> f64 {
    HASH_EXTRA_CAPACITY
}

fn seg_size() -> i32 {
    SEG_SIZE
}

fn segments() -> i32 {
    SEGMENTS
}

fn eviction() -> Eviction {
    EVICTION
}

fn merge_target() -> usize {
    MERGE_TARGET
}

fn merge_max() -> usize {
    MERGE_MAX
}

fn compact_target() -> usize {
    COMPACT_TARGET
}

// definitions
#[derive(Serialize, Deserialize, Debug)]
pub struct SegCacheConfig {
    #[serde(default = "hash_power")]
    hash_power: u8,
    #[serde(default = "hash_extra_capacity")]
    hash_extra_capacity: f64,
    #[serde(default = "seg_size")]
    seg_size: i32,
    #[serde(default = "segments")]
    segments: i32,
    #[serde(default = "eviction")]
    eviction: Eviction,
    #[serde(default = "merge_target")]
    merge_target: usize,
    #[serde(default = "merge_max")]
    merge_max: usize,
    #[serde(default = "compact_target")]
    compact_target: usize,
}

impl Default for SegCacheConfig {
    fn default() -> Self {
        Self {
            hash_power: hash_power(),
            hash_extra_capacity: hash_extra_capacity(),
            seg_size: seg_size(),
            segments: segments(),
            eviction: eviction(),
            merge_target: merge_target(),
            merge_max: merge_max(),
            compact_target: compact_target(),
        }
    }
}

// implementation
impl SegCacheConfig {
    pub fn hash_power(&self) -> u8 {
        self.hash_power
    }

    pub fn hash_extra_capacity(&self) -> f64 {
        self.hash_extra_capacity
    }

    pub fn seg_size(&self) -> i32 {
        self.seg_size
    }

    pub fn segments(&self) -> i32 {
        self.segments
    }

    pub fn eviction(&self) -> Eviction {
        self.eviction
    }

    pub fn merge_target(&self) -> usize {
        self.merge_target
    }

    pub fn merge_max(&self) -> usize {
        self.merge_max
    }

    pub fn compact_target(&self) -> usize {
        self.compact_target
    }
}
