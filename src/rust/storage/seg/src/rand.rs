// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Random number generator initialization

pub use inner::*;

pub use rand::Rng as RandRng;
pub use rand::RngCore as RandRngCore;

#[cfg(test)]
mod inner {
    use rand::SeedableRng;

    pub type Random = rand_xoshiro::Xoshiro256PlusPlus;

    // A very fast PRNG which is appropriate for testing
    pub fn rng() -> Random {
        rand_xoshiro::Xoshiro256PlusPlus::seed_from_u64(0)
    }
}

#[cfg(not(test))]
mod inner {
    use rand::SeedableRng;

    pub type Random = rand_chacha::ChaCha20Rng;

    // A cryptographically secure RNG using the ChaCha algorithm. Appropriate
    // for production.
    pub fn rng() -> Random {
        rand_chacha::ChaCha20Rng::from_entropy()
    }
}
