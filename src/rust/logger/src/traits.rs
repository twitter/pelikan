// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

/// An extended version of the `Log` trait that provides a convenience function
/// for starting the logger.
pub trait LogEx: 'static + log::Log + Sized {
    fn level_filter(&self) -> LevelFilter;

    fn start(self) {
        let level_filter = self.level_filter();
        log::set_boxed_logger(Box::new(self))
            .map(|()| log::set_max_level(level_filter))
            .expect("failed to start logger");
    }
}