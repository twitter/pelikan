extern crate cdb_rs;
extern crate rand;
#[macro_use] extern crate clap;
use clap::ArgMatches;
use rand::{thread_rng,Rng};
use rand::distributions::Alphanumeric;
use std::io;
use std::io::Write;


fn alpha(min: usize, max: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(
        if min == max { min } else { thread_rng().gen_range(min, max) }
    ).collect()
}

const MIN_KEY_SIZE: usize = 8;
const MIN_VAL_SIZE: usize = 8;
const MAX_KEY_SIZE: usize = 256;
const MAX_VAL_SIZE: usize = 1024 * 1024;
const DEFAULT_N_RECORDS: usize = 1000;

fn main() {
    let matches: ArgMatches = clap_app!(randoread =>
        (version: "0.1.0")
        (author: "Jonathan Simms <jsimms@twitter.com>")
        (@arg ENTRIES: -n --nrecords [N] "number of records to generate")
        (@arg MINKEY: -k --minkey [N] "min key size")
        (@arg MAXKEY: -K --maxkey [N] "max key size")
        (@arg MINVAL: -v --minval [N] "min val size")
        (@arg MAXVAL: -V --maxval [N] "max val size")
    ).get_matches();

    let argval = |argname,default| {
        match matches.value_of(argname) {
            Some(str) => str.parse::<usize>().unwrap(),
            None => default,
        }
    };

    let min_k: usize = argval("MINKEY", MIN_KEY_SIZE);
    let max_k: usize = argval("MAXKEY", MAX_KEY_SIZE);
    let min_v: usize = argval("MINVAL", MIN_VAL_SIZE);
    let max_v: usize = argval("MAXVAL", MAX_VAL_SIZE);
    let num_rec: usize = argval("ENTRIES", DEFAULT_N_RECORDS);

    let mut i: usize = 0;
    while i < num_rec {
        let k: String = alpha(min_k, max_k);
        let v: String = alpha(min_v, max_v);

        writeln!(io::stdout(), "+{},{}:{}->{}", k.len(), v.len(), k, v).unwrap();
        i += 1;
    }
    writeln!(io::stdout(), "").unwrap();
}
