// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::time::Duration;
use logger::*;

macro_rules! command {
    ($($arg:tt)*) => (
        // we choose error level here because it is the lowest level and will
        // not be filtered unless the level filter is set to `off`
        error!(target: "command", $($arg)*);
    )
}

macro_rules! noplog {
    ($($arg:tt)*) => (
        // we choose error level here because it is the lowest level and will
        // not be filtered unless the level filter is set to `off`
        error!(target: "noplog", $($arg)*);
    )
}

fn main() {
    let default = LogBuilder::new()
        .output(Box::new(Stdout::new()))
        .build()
        .expect("failed to initialize default log");

    let command = LogBuilder::new()
        .output(Box::new(
            File::new("command.log", "command.old", 100).expect("failed to create file log"),
        ))
        .format(klog_format)
        .build()
        .expect("failed to initialize command log");

    let noplog = NopLogBuilder::new().build();

    let mut drain = MultiLogBuilder::new()
        .default(default)
        .add_target("command", command)
        .add_target("noplog", noplog)
        .build()
        .start();

    std::thread::spawn(move || loop {
        let _ = drain.flush();
        std::thread::sleep(Duration::from_millis(100));
    });

    error!("error");
    warn!("warning");
    info!("info");
    debug!("debug");
    trace!("trace");

    command!("\"get 0\" 0 0");

    noplog!("this won't get displayed");

    std::thread::sleep(Duration::from_millis(1000));
}
