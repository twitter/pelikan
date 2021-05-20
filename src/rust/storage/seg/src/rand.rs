// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Random number generator initialization

pub use inner::*;

pub use rand::Rng as RandRng;
pub use rand::RngCore as RandRngCore;

// TODO(bmartin): this might be dangerous to gate with the `cfg(test)`. The
// intent is to allow predictable testing. Many of the behaviors are
// probabilistic during testing - insertion order, eviction order, etc. We
// should prefer consistent testing results rather than flaky testing.
//
// The risk comes from not proving that the code works with the normal RNG
// This can be mitigated by adding a smoketest using a release build, which is
// a good idea anyway.

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
    // for runtime.
    pub fn rng() -> Random {
        rand_chacha::ChaCha20Rng::from_entropy()
    }
}
