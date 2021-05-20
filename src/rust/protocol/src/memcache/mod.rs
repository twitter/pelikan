// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

mod entry;
pub mod data;
mod storage;

pub use entry::MemcacheEntry;
pub use storage::MemcacheStorage;
// pub use storage::MemcacheEntry;