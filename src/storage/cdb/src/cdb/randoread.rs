use bytes::Bytes;
use rand::{thread_rng, Rng};
use std::time::{Duration, Instant};

use super::Result;

#[derive(Clone, Copy, Debug)]
pub struct RandoConfig {
    // a number from [0.0, 1.0) that controls the likelihood that any
    // particular key will be chosen to test with. 0.3 by default
    pub probability: f32,

    // max number of keys to test with, defaults to 10k
    pub max_keys: usize,

    // number of iterations to do: default 10k
    pub iters: u64,

    pub use_mmap: bool,

    pub use_stdio: bool,
}

impl RandoConfig {
    pub fn new() -> RandoConfig {
        RandoConfig {
            probability: 0.3,
            max_keys: 10_000,
            iters: 10_000,
            use_mmap: false,
            use_stdio: false,
        }
    }

    pub fn probability<'a>(&'a mut self, prob: f32) -> &'a mut RandoConfig {
        self.probability = prob;
        self
    }

    pub fn max_keys<'a>(&'a mut self, max: usize) -> &'a mut RandoConfig {
        self.max_keys = max;
        self
    }

    pub fn iters<'a>(&'a mut self, num_iter: u64) -> &'a mut RandoConfig {
        self.iters = num_iter;
        self
    }

    pub fn use_mmap<'a>(&'a mut self, b: bool) -> &'a mut RandoConfig {
        self.use_mmap = b;
        self
    }

    pub fn use_stdio<'a>(&'a mut self, b: bool) -> &'a mut RandoConfig {
        self.use_stdio = b;
        self
    }
}


pub fn run(db: &super::CDB, config: &RandoConfig) -> Result<Duration> {
    let mut rng = thread_rng();

    let mut keys = {
        let mut ks: Vec<Bytes> =
            db.kvs_iter()?
                .map(|kv| kv.unwrap().k)
                .collect();

        ks.shrink_to_fit();
        ks
    };

    rng.shuffle(&mut keys);

    let keyiter =
        keys.iter()
            .take(config.iters as usize)
            .cycle()
            .take(config.iters as usize);

    eprintln!("starting test using {} sampled keys", config.iters);
    let start = Instant::now();

    let mut hit = 0;
    let mut miss = 0;
    let mut bytes = 0;

    let mut buf: Vec<u8> = Vec::with_capacity(1024 * 1024);

    for k in keyiter {
        buf.clear();
        if db.get(&k[..], &mut buf)?.is_some() {
            hit += 1;
            bytes += buf.len();
        } else {
            miss += 1
        }
    }

    let hitrate = (hit as f64 / config.iters as f64) * 100.0;

    debug!(
        "hit: {}, miss: {}, ratio: {:.3}%, bytes: {}",
        hit,
        miss,
        hitrate,
        bytes
    );

    Ok(start.elapsed())
}
