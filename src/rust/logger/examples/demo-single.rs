// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::time::Duration;
use logger::*;

fn main() {
    let log = LogBuilder::new()
        .output(Box::new(Stdout::new()))
        .build()
        .expect("failed to initialize log");

    let mut drain = log.start();

    std::thread::spawn(move || loop {
        let _ = drain.flush();
        std::thread::sleep(Duration::from_millis(100));
    });

    error!("error");
    warn!("warning");
    info!("info");
    debug!("debug");
    trace!("trace");

    std::thread::sleep(Duration::from_millis(1000));
}
