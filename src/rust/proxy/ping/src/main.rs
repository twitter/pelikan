// Copyright 2022 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[macro_use]
extern crate logger;

use backtrace::Backtrace;
use clap::App;
use clap::Arg;
use config::PingproxyConfig;
use metrics::*;
use pingproxy::Pingproxy;

use proxy::PERCENTILES;

fn main() {
    // custom panic hook to terminate whole process after unwinding
    std::panic::set_hook(Box::new(|s| {
        error!("{}", s);
        println!("{:?}", Backtrace::new());
        std::process::exit(101);
    }));

    // parse command line options
    let matches = App::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .version_short("v")
        .long_about(
            "A Pelikan proxy server which speaks the ASCII `ping` protocol. It \
            accepts connections on the listening port, routing requests to the \
            backend servers and responses back to clients.",
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
        .get_matches();

    // output stats descriptions and exit if the `stats` option was provided
    if matches.is_present("stats") {
        println!("{:<31} {:<15} DESCRIPTION", "NAME", "TYPE");

        let mut metrics = Vec::new();

        for metric in &metrics::common::metrics::metrics() {
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
        match PingproxyConfig::load(file) {
            Ok(c) => c,
            Err(e) => {
                println!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        Default::default()
    };

    // launch proxy
    Pingproxy::new(config).wait()
}
