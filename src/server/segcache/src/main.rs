// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Segcache is an implementation of a cache backend that implements a subset of
//! the Memcache ASCII protocol and is backed with segment based storage. By
//! grouping items with a similar TTL, it is able to provide efficient eager
//! expiration.
//!
//! More details about the benefits of this design can be found in this
//! [blog post](https://twitter.github.io/pelikan/2021/segcache.html).
//!
//! Running this binary is the primary way of using Segcache.

#[macro_use]
extern crate logger;

use backtrace::Backtrace;
use clap::{App, Arg};
use config::SegcacheConfig;
use pelikan_segcache_rs::Segcache;
use rustcommon_metrics::*;
use server::PERCENTILES;

/// The entry point into the running Segcache instance. This function parses the
/// command line options, loads the configuration, and launches the core
/// threads.
fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        eprintln!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // parse command line options
    let matches = App::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .version_short("v")
        .long_about(
            "One of the unified cache backends implemented in Rust. It \
            uses segment-based storage to cache key/val pairs. It speaks the \
            memcached ASCII protocol and supports some ASCII memcached \
            commands.",
        )
        .arg(
            Arg::with_name("stats")
                .short("s")
                .long("stats")
                .help("List all metrics in stats")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("CONFIG")
                .help("Server configuration file")
                .index(1),
        )
        .arg(
            Arg::with_name("print-config")
                .help("List all options in config")
                .long("config")
                .short("c"),
        )
        .get_matches();

    // output stats descriptions and exit if the `stats` option was provided
    if matches.is_present("stats") {
        println!("{:<31} {:<15} DESCRIPTION", "NAME", "TYPE");

        let mut metrics = Vec::new();

        for metric in &rustcommon_metrics::metrics() {
            let any = match metric.as_any() {
                Some(any) => any,
                None => {
                    continue;
                }
            };

            if any.downcast_ref::<Counter>().is_some() {
                metrics.push(format!("{:<31} counter", metric.name()));
            } else if any.downcast_ref::<Gauge>().is_some() {
                metrics.push(format!("{:<31} gauge", metric.name()));
            } else if any.downcast_ref::<Heatmap>().is_some() {
                for (label, _) in PERCENTILES {
                    let name = format!("{}_{}", metric.name(), label);
                    metrics.push(format!("{:<31} percentile", name));
                }
            } else {
                continue;
            }
        }

        metrics.sort();
        for metric in metrics {
            println!("{}", metric);
        }
        std::process::exit(0);
    }

    // load config from file
    let config = if let Some(file) = matches.value_of("CONFIG") {
        match SegcacheConfig::load(file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("unable to load config file: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        Default::default()
    };

    if matches.is_present("print-config") {
        config.print();
        std::process::exit(0);
    }

    // launch segcache
    match Segcache::new(config) {
        Ok(segcache) => segcache.wait(),
        Err(e) => {
            eprintln!("error launching segcache: {}", e);
            std::process::exit(1);
        }
    }
}
