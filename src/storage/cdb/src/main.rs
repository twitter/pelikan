extern crate cdb_rs;
#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use] extern crate clap;
extern crate memmap;

use std::time::Duration;

use cdb_rs::cdb;
use cdb_rs::cdb::storage::SliceFactory;
use cdb_rs::cdb::randoread::RandoConfig;

use cdb_rs::cdb::Result;

use clap::ArgMatches;

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

    let d = cdb::randoread::run(&db, &config)?;
    let d2f = dur2sec(&d);
    let rate = config.iters as f64 / d2f;

    info!("{} iters in {:.3} sec, {:.3} op/sec", config.iters, d2f, rate);
    Ok(())
}

fn main() {
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

    std::process::exit(
        match randoread(filename, &rc) {
            Ok(_) => 0,
            Err(err) => {
                eprintln!("error: {:?}", err);
                1
            }
        }
    );
}
