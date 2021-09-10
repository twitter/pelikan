// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::*;

pub struct LogReceiver {
    pub(crate) debug: FileLogReceiver,
    pub(crate) command: Option<FileLogReceiver>,
}

impl LogReceiver {
    pub fn flush(&mut self) {
        self.debug.flush();
        if let Some(command) = &mut self.command {
            command.flush()
        };
    }
}
