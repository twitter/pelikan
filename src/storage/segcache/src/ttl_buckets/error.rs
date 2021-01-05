// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("item is oversized")]
    ItemOversized,
    #[error("ttl bucket expansion failed, no free segments")]
    NoFreeSegments,
}
