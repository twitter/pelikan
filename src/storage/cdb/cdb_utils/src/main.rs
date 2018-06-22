extern crate cdb_rs;
#[macro_use]
extern crate log;
extern crate env_logger;
extern crate bytes;
extern crate rand;
#[macro_use] extern crate clap;

use rand::{thread_rng,Rng};
use clap::ArgMatches;
use bytes::Bytes;

use std::time::{Duration, Instant};

use cdb_rs::cdb;
use cdb_rs::cdb::storage::SliceFactory;
use cdb_rs::cdb::{Result, CDB};


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


pub fn run_rando_read(db: &CDB, config: &RandoConfig) -> Result<Duration> {
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

fn dur2sec(d: &Duration) -> f64  {
    d.as_secs() as f64 + (d.subsec_nanos() as f64 * 1e-9)
}

fn randoread(filename: &str, config: &RandoConfig) -> Result<()> {
    let sf: SliceFactory;

    let db =
        if config.use_mmap {
            cdb::CDB::new(SliceFactory::make_map(filename)?)
        } else {
            {
                if config.use_stdio {
                    sf = SliceFactory::make_filewrap(filename)?;
                } else {
                    sf = SliceFactory::load(filename)?;
                }
            }
            cdb::CDB::new(sf)
        };

    let d = run_rando_read(&db, &config)?;
    let d2f = dur2sec(&d);
    let rate = config.iters as f64 / d2f;

    info!("{} iters in {:.3} sec, {:.3} op/sec", config.iters, d2f, rate);
    Ok(())
}

fn main() -> Result<()> {
    match env_logger::try_init() {
        Ok(_) => "",    // yay great
        Err(_) => "",   // wtfever
    };

    let matches: ArgMatches = clap_app!(randoread =>
        (version: "0.1.0")
        (author: "Jonathan Simms <jsimms@twitter.com>")
        (@arg ITERS: -i --iters [N] "number of iterations to do")
        (@arg PROB: -p --probability [N] "probability filter for keys, float [0.0, 1.0)")
        (@arg NKEYS: -k --numkeys [N] "max number of keys to test with")
        (@arg MMAP: -M --mmap conflicts_with[STDIO] "use alternate mmap implmeentation (experimental on linux)")
        (@arg STDIO: -S --stdio conflicts_with[MMAP] "use stdio implementation")
        (@arg INPUT: +required "the .cdb file to test")
    ).get_matches();

    let mut rc = RandoConfig::new();

    if let Some(val) = matches.value_of("ITERS") {
        rc.iters(val.parse().unwrap());
    }

    if let Some(p) = matches.value_of("PROB") {
        rc.probability(p.parse().unwrap());
    }

    if let Some(p) = matches.value_of("NKEYS") {
        rc.max_keys(p.parse().unwrap());
    }

    match matches.occurrences_of("MMAP") {
        0 => rc.use_mmap(false),
        _ => rc.use_mmap(true),
    };

    match matches.occurrences_of("STDIO") {
        0 => rc.use_stdio(false),
        _ => rc.use_stdio(true),
    };


    let filename = matches.value_of("INPUT").unwrap();

    debug!("using config: {:?}", rc);
    randoread(filename, &rc)
}