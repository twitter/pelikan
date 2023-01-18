// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use bloom::BloomFilter;
use config::BloomConfig;

use crate::EntryStore;

mod http;

/// A wrapper around [`bloom::BloomFilter`] which implements `EntryStore`
/// protocol traits.
pub struct Bloom {
    data: BloomFilter<[u8]>,
}

impl Bloom {
    /// Create a bloom filter storage based on the config.
    pub fn new<T: BloomConfig>(config: &T) -> Result<Self, std::io::Error> {
        // TODO: Validate the config here and return an error.

        let config = config.bloom();
        Ok(Self {
            data: BloomFilter::new(config.size * 8, config.hashes),
        })
    }
}

impl EntryStore for Bloom {
    fn clear(&mut self) {
        self.data.clear();
    }
}
