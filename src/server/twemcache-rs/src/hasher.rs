// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use ahash::RandomState;
use core::hash::BuildHasher;

pub struct CacheHasher {
    inner: ahash::RandomState,
}

impl Default for CacheHasher {
    fn default() -> Self {
        let inner = RandomState::with_seeds(
            0xbb8c484891ec6c86,
            0x0522a25ae9c769f9,
            0xeed2797b9571bc75,
            0x4feb29c1fbbd59d0,
        );
        Self { inner }
    }
}

impl BuildHasher for CacheHasher {
    type Hasher = ahash::AHasher;

    fn build_hasher(&self) -> Self::Hasher {
        self.inner.build_hasher()
    }
}
