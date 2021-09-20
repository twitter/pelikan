// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use config::KlogConfig;
use config::DebugConfig;
pub use log::*;
pub type LogBuffer = Vec<u8>;

mod format;
mod multi;
mod outputs;
mod single;
mod traits;

pub use format::*;
pub use multi::*;
pub use outputs::*;
pub use single::*;
pub use traits::*;

// for convenience include these
use mpmc::Queue;
use rustcommon_time::recent_local;

////////////////////////////////////////////////////////////////////////////////
// TODO(bmartin): everything below is Pelikan specific, and should be factored
// out into a helper when we move this crate into rustcommon
////////////////////////////////////////////////////////////////////////////////

#[macro_export]
macro_rules! klog {
    ($($arg:tt)*) => (
        error!(target: "klog", $($arg)*);
    )
}

pub fn configure_logging(debug_config: &DebugConfig, klog_config: &KlogConfig) -> MultiLogDrain {
	let debug_output: Box<dyn Output> = if let Some(file) = debug_config.log_file() {
	    let backup = debug_config
	        .log_backup()
	        .unwrap_or(format!("{}.old", file));
	    Box::new(
	        File::new(&file, &backup, debug_config.log_max_size())
	            .expect("failed to open debug log file"),
	    )
	} else {
	    Box::new(Stdout::new())
	};

	let debug_log = LogBuilder::new()
	    .output(debug_output)
	    .build()
	    .expect("failed to initialize debug log");

	let mut log_builder = MultiLogBuilder::new()
		.level_filter(debug_config.log_level().to_level_filter())
		.default(debug_log);

	if let Some(file) = klog_config.file() {
        let backup = klog_config.backup().unwrap_or(format!("{}.old", file));
        let output = Box::new(
            File::new(&file, &backup, klog_config.max_size())
                .expect("failed to open klog file"),
        );
        let klog = LogBuilder::new()
            .output(output)
            .format(klog_format)
            .build()
            .expect("failed to initialize klog");
        log_builder = log_builder.add_target("klog", klog);
    }

    let (logger, log_drain) = log_builder.build();

    logger.start();

    log_drain
}
