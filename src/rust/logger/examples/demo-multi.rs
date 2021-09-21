// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::time::Duration;
use logger::*;

macro_rules! command {
    ($($arg:tt)*) => (
        error!(target: "command", $($arg)*);
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

    let log = MultiLogBuilder::new()
        .default(default)
        .add_target("command", command)
        .build();

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

    command!("\"get 0\" 0 0");

    std::thread::sleep(Duration::from_millis(1000));
}
