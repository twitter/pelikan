// Copyright 2021 Twitter, Inc.
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! A collection of protocol implementations which implement a set of common
//! traits so that the a server implementation can easily switch between
//! protocol implementations.

// TODO(bmartin): this crate should probably be split into one crate per
// protocol to help separate the metrics namespaces.

pub use protocol_common::*;

mod ping;

pub use ping::*;

common::metrics::test_no_duplicates!();
