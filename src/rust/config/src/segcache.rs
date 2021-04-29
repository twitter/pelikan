// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use serde::{Deserialize, Serialize};

const MB: usize = 1024 * 1024;

// defaults for hashtable
const HASH_POWER: u8 = 16;
const OVERFLOW_FACTOR: f64 = 1.0;

// default heap/segment sizing
const HEAP_SIZE: usize = 64 * MB;
const SEGMENT_SIZE: i32 = MB as i32;

// default eviction strategy
const EVICTION: Eviction = Eviction::Merge;

// related to merge eviction
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

fn overflow_factor() -> f64 {
    OVERFLOW_FACTOR
}

fn heap_size() -> usize {
    HEAP_SIZE
}

fn segment_size() -> i32 {
    SEGMENT_SIZE
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
    #[serde(default = "overflow_factor")]
    overflow_factor: f64,
    #[serde(default = "heap_size")]
    heap_size: usize,
    #[serde(default = "segment_size")]
    segment_size: i32,
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
            overflow_factor: overflow_factor(),
            heap_size: heap_size(),
            segment_size: segment_size(),
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

    pub fn overflow_factor(&self) -> f64 {
        self.overflow_factor
    }

    pub fn heap_size(&self) -> usize {
        self.heap_size
    }

    pub fn segment_size(&self) -> i32 {
        self.segment_size
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
