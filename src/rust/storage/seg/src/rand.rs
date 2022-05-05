// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Random number generator initialization

pub use inner::*;

pub use rand::Rng as RandRng;
pub use rand::RngCore as RandRngCore;

use core::cell::UnsafeCell;
use rand::Error;
use rand::RngCore;
use std::rc::Rc;

pub struct ThreadRng {
    // Rc is explicitly !Send and !Sync
    rng: Rc<UnsafeCell<Random>>,
}

thread_local!(
    // We require Rc<..> to avoid premature freeing when thread_rng is used
    // within thread-local destructors. See #968.
    static THREAD_RNG_KEY: Rc<UnsafeCell<Random>> = {
        let rng = rng();
        Rc::new(UnsafeCell::new(rng))
    }
);

pub fn thread_rng() -> ThreadRng {
    let rng = THREAD_RNG_KEY.with(|t| t.clone());
    ThreadRng { rng }
}

impl RngCore for ThreadRng {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.try_fill_bytes(dest)
    }
}

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
