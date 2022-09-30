// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::units::MB;

use serde::{Deserialize, Serialize};

const BLOOM_DEFAULT_SIZE: usize = 16 * MB;
const BLOOM_DEFAULT_HASHES: usize = 64;

fn size() -> usize {
    BLOOM_DEFAULT_SIZE
}

fn hashes() -> usize {
    BLOOM_DEFAULT_HASHES
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Bloom {
    /// The size of the bloom filter in bytes.
    #[serde(default = "size")]
    pub size: usize,

    /// The number of hash functions that are evaluated for each value inserted.F
    #[serde(default = "hashes")]
    pub hashes: usize,
}

impl Default for Bloom {
    fn default() -> Self {
        Self {
            size: BLOOM_DEFAULT_SIZE,
            hashes: BLOOM_DEFAULT_HASHES,
        }
    }
}

pub trait BloomConfig {
    fn bloom(&self) -> &Bloom;
}
